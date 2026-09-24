using System.Runtime.InteropServices;

namespace OpenSwitch;

public sealed class GlobalHotkeyService : NativeWindow, IDisposable
{
    private const int FirstHotkeyId = 100;
    private readonly Dictionary<int, string> actionsById = [];
    private readonly List<int> registeredHotkeys = [];
    private readonly HashSet<string> registeredShortcuts = new(StringComparer.OrdinalIgnoreCase);
    private int nextHotkeyId = FirstHotkeyId;
    private bool handleCreated;

    public GlobalHotkeyService(IEnumerable<HotkeyBinding> bindings)
    {
        CreateHandle(new CreateParams
        {
            Caption = "OpenSwitchHotkeyWindow"
        });
        handleCreated = true;
        Reconfigure(bindings);
    }

    public event Action<string>? HotkeyPressed;

    public List<string> RegistrationErrors { get; } = [];

    public bool IsActionAvailable(string action)
    {
        return actionsById.Values.Contains(action, StringComparer.OrdinalIgnoreCase);
    }

    public bool IsCurrentWordAvailable => IsActionAvailable("CurrentWord");

    public bool IsSelectionAvailable => IsActionAvailable("SelectedText");

    public void Reconfigure(IEnumerable<HotkeyBinding> bindings)
    {
        UnregisterAll();
        RegistrationErrors.Clear();
        foreach (var binding in bindings ?? [])
        {
            if (!binding.Enabled || string.IsNullOrWhiteSpace(binding.Shortcut))
            {
                continue;
            }

            RegisterHotkey(binding);
        }
    }

    protected override void WndProc(ref Message message)
    {
        if (message.Msg == NativeMethods.WM_HOTKEY
            && actionsById.TryGetValue(message.WParam.ToInt32(), out var action))
        {
            HotkeyPressed?.Invoke(action);
            return;
        }

        base.WndProc(ref message);
    }

    public void Dispose()
    {
        if (handleCreated)
        {
            UnregisterAll();
            DestroyHandle();
            handleCreated = false;
        }

        GC.SuppressFinalize(this);
    }

    private void RegisterHotkey(HotkeyBinding binding)
    {
        if (!TryParseShortcut(binding.Shortcut, out var modifiers, out var virtualKey))
        {
            RegistrationErrors.Add($"Invalid shortcut: {binding.Shortcut}");
            return;
        }

        if (!registeredShortcuts.Add(binding.Shortcut))
        {
            RegistrationErrors.Add($"Duplicate shortcut: {binding.Shortcut}");
            return;
        }

        var hotkeyId = nextHotkeyId++;
        if (NativeMethods.RegisterHotKey(Handle, hotkeyId, modifiers | NativeMethods.MOD_NOREPEAT, virtualKey))
        {
            actionsById.Add(hotkeyId, binding.Action);
            registeredHotkeys.Add(hotkeyId);
            return;
        }

        registeredShortcuts.Remove(binding.Shortcut);
        RegistrationErrors.Add(
            $"Unable to register {binding.Shortcut} for {binding.Action} (code {Marshal.GetLastWin32Error()}).");
    }

    private void UnregisterAll()
    {
        foreach (var hotkeyId in registeredHotkeys)
        {
            NativeMethods.UnregisterHotKey(Handle, hotkeyId);
        }

        registeredHotkeys.Clear();
        actionsById.Clear();
        registeredShortcuts.Clear();
    }

    private static bool TryParseShortcut(string shortcut, out uint modifiers, out ushort virtualKey)
    {
        modifiers = 0;
        virtualKey = 0;
        var tokens = shortcut.Split('+', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (tokens.Length == 0)
        {
            return false;
        }

        foreach (var token in tokens)
        {
            switch (token.ToLowerInvariant())
            {
                case "ctrl":
                    modifiers |= NativeMethods.MOD_CONTROL;
                    continue;
                case "alt":
                    modifiers |= NativeMethods.MOD_ALT;
                    continue;
                case "shift":
                    modifiers |= NativeMethods.MOD_SHIFT;
                    continue;
                case "win":
                case "windows":
                    modifiers |= NativeMethods.MOD_WIN;
                    continue;
            }

            if (!Enum.TryParse<Keys>(token, ignoreCase: true, out var key)
                || key == Keys.None
                || (ushort)key > 0xFF)
            {
                return false;
            }

            virtualKey = (ushort)key;
        }

        return virtualKey != 0;
    }
}
