using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace OpenSwitch;

public sealed class AutoLayoutService : NativeWindow, IDisposable
{
    private const int WhKeyboardLl = 13;
    private const int WmKeyDown = 0x0100;
    private const int WmKeyUp = 0x0101;
    private const int WmSysKeyDown = 0x0104;
    private const int WmSysKeyUp = 0x0105;
    private const ushort VkRightControl = 0xA3;
    private const ushort VkLeftShift = 0xA0;
    private const ushort VkRightShift = 0xA1;
    private const ushort VkBack = 0x08;
    private const ushort VkDelete = 0x2E;
    private const ushort VkLeft = 0x25;
    private const ushort VkUp = 0x26;
    private const ushort VkRight = 0x27;
    private const ushort VkDown = 0x28;
    private const ushort VkLayout = 0xFF;
    private const ushort VkReturn = 0x0D;
    private const ushort VkTab = 0x09;
    private const ushort VkSpace = 0x20;

    private readonly LowLevelKeyboardProc keyboardProc;
    private readonly StringBuilder currentWord = new();
    private readonly TextTransformer textTransformer;
    private readonly AutoReplacementService autoReplacementService;
    private AppSettings settings;
    private IntPtr hookHandle;
    private bool rightControlDown;
    private bool rightControlShortcut;
    private bool shiftHandled;
    private bool suppressEvaluation;

    public AutoLayoutService(AppSettings settings, TextTransformer textTransformer)
    {
        this.settings = settings;
        this.textTransformer = textTransformer;
        autoReplacementService = new AutoReplacementService(settings);
        keyboardProc = HookCallback;
        CreateHandle(new CreateParams
        {
            Caption = "OpenSwitchAutoLayoutWindow"
        });
        ApplySettings(settings);
    }

    public string LastError { get; private set; } = string.Empty;

    public void ApplySettings(AppSettings updatedSettings)
    {
        settings = updatedSettings;
        autoReplacementService.ApplySettings(updatedSettings);
        if (settings.AutoSwitchEnabled)
        {
            Start();
        }
        else
        {
            Stop();
        }
    }

    public void Dispose()
    {
        Stop();
        DestroyHandle();
        GC.SuppressFinalize(this);
    }

    private void Start()
    {
        if (hookHandle != IntPtr.Zero)
        {
            return;
        }

        hookHandle = SetWindowsHookEx(WhKeyboardLl, keyboardProc, GetModuleHandle(null), 0);
        LastError = hookHandle == IntPtr.Zero
            ? new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()).Message
            : string.Empty;
    }

    private void Stop()
    {
        if (hookHandle == IntPtr.Zero)
        {
            return;
        }

        UnhookWindowsHookEx(hookHandle);
        hookHandle = IntPtr.Zero;
        currentWord.Clear();
        rightControlDown = false;
        rightControlShortcut = false;
        shiftHandled = false;
        suppressEvaluation = false;
    }

    private IntPtr HookCallback(int code, IntPtr wParam, IntPtr lParam)
    {
        if (code >= 0 && TryGetKeyboardData(lParam, out var data))
        {
            var message = wParam.ToInt32();
            var keyDown = message is WmKeyDown or WmSysKeyDown;
            var keyUp = message is WmKeyUp or WmSysKeyUp;
            if (keyDown)
            {
                HandleKeyDown((ushort)data.vkCode);
            }
            else if (keyUp)
            {
                HandleKeyUp((ushort)data.vkCode);
            }
        }

        return CallNextHookEx(IntPtr.Zero, code, wParam, lParam);
    }

    private void HandleKeyDown(ushort virtualKey)
    {
        if (IsExcludedProgram())
        {
            currentWord.Clear();
            return;
        }

        if (ShouldSuppressEvaluation(virtualKey))
        {
            currentWord.Clear();
            suppressEvaluation = true;
            return;
        }

        if (settings.SwitchByRightControl && virtualKey == VkRightControl)
        {
            rightControlDown = true;
            rightControlShortcut = true;
            return;
        }

        if (rightControlDown && virtualKey is not (VkRightControl or 0xA2))
        {
            rightControlShortcut = false;
        }

        if (settings.AdditionalShiftSwitch && virtualKey is VkLeftShift or VkRightShift && !shiftHandled)
        {
            shiftHandled = true;
            LayoutSwitchService.Toggle();
            return;
        }

        if (virtualKey == VkBack)
        {
            if (settings.DoNotSwitchAfterBackspace)
            {
                suppressEvaluation = true;
            }

            if (currentWord.Length > 0)
            {
                currentWord.Length--;
            }

            return;
        }

        if (virtualKey is VkReturn or VkTab or VkSpace)
        {
            EvaluateCurrentWord(virtualKey);
            return;
        }

        if (TryGetTypedCharacter(virtualKey, out var character)
            && char.IsLetterOrDigit(character))
        {
            currentWord.Append(character);
        }
    }

    private void HandleKeyUp(ushort virtualKey)
    {
        if (virtualKey == VkRightControl)
        {
            if (rightControlDown && rightControlShortcut && !IsExcludedProgram())
            {
                LayoutSwitchService.Toggle();
            }

            rightControlDown = false;
            rightControlShortcut = false;
            return;
        }

        if (virtualKey is VkLeftShift or VkRightShift)
        {
            shiftHandled = false;
        }
    }

    private void EvaluateCurrentWord(ushort delimiter)
    {
        if (currentWord.Length == 0)
        {
            suppressEvaluation = false;
            return;
        }

        if (suppressEvaluation)
        {
            suppressEvaluation = false;
            currentWord.Clear();
            return;
        }

        var word = currentWord.ToString();
        currentWord.Clear();
        var fixedWord = TextFixer.Fix(word, settings);
        if (!string.Equals(fixedWord, word, StringComparison.Ordinal))
        {
            textTransformer.TryReplaceCurrentWord(fixedWord, out _);
            return;
        }

        var triggerKey = delimiter switch
        {
            VkReturn => "Enter",
            VkTab => "Tab",
            _ => "Space"
        };
        if (autoReplacementService.TryGetReplacement(word, triggerKey, out var replacement))
        {
            textTransformer.TryReplaceCurrentWord(replacement, out _);
            return;
        }

        if (settings.DoNotSwitchOnTabEnter && delimiter is VkReturn or VkTab)
        {
            return;
        }

        var action = FindRuleAction(word);
        if (settings.DoNotSwitchBetweenRussianEnglish
            || string.Equals(action, "DoNotSwitch", StringComparison.OrdinalIgnoreCase))
        {
            return;
        }

        if (action is not null || AutoSwitchHeuristics.ShouldSwitch(word))
        {
            LayoutSwitchService.Toggle();
        }
    }

    private bool ShouldSuppressEvaluation(ushort virtualKey)
    {
        return virtualKey switch
        {
            VkLeft => settings.DoNotSwitchAfterNavigation || settings.DoNotSwitchAfterLeftArrow,
            VkRight => settings.DoNotSwitchAfterNavigation || settings.DoNotSwitchAfterRightArrow,
            VkUp => settings.DoNotSwitchAfterNavigation || settings.DoNotSwitchAfterUpArrow,
            VkDown => settings.DoNotSwitchAfterNavigation || settings.DoNotSwitchAfterDownArrow,
            VkDelete => settings.DoNotSwitchAfterDelete,
            VkLayout => settings.DoNotSwitchAfterLayoutKey,
            _ => false
        };
    }

    private bool IsExcludedProgram()
    {
        return (settings.ProgramRules.Count > 0 || settings.DoNotInteractWithProgramRules)
            && ProgramRuleMatcher.IsExcluded(settings);
    }

    private string? FindRuleAction(string word)
    {
        foreach (var rule in settings.Rules)
        {
            if (!rule.Enabled)
            {
                continue;
            }

            if (!string.IsNullOrWhiteSpace(rule.Regex))
            {
                try
                {
                    if (System.Text.RegularExpressions.Regex.IsMatch(word, rule.Regex))
                    {
                        return rule.Action;
                    }
                }
                catch (ArgumentException)
                {
                }
            }

            if (!string.IsNullOrWhiteSpace(rule.Condition)
                && word.Contains(rule.Condition, StringComparison.OrdinalIgnoreCase))
            {
                return rule.Action;
            }
        }

        return null;
    }

    private bool TryGetTypedCharacter(ushort virtualKey, out char character)
    {
        character = default;
        var foregroundWindow = NativeMethods.GetForegroundWindow();
        if (foregroundWindow == IntPtr.Zero)
        {
            return false;
        }

        var threadId = GetWindowThreadProcessId(foregroundWindow, out _);
        var keyboardLayout = GetKeyboardLayout(threadId);
        var keyboardState = new byte[256];
        if (!GetKeyboardState(keyboardState))
        {
            return false;
        }

        var buffer = new char[8];
        var result = ToUnicodeEx(
            virtualKey,
            0,
            keyboardState,
            buffer,
            buffer.Length,
            0,
            keyboardLayout);
        if (result <= 0)
        {
            return false;
        }

        character = buffer[0];
        return true;
    }

    private static bool TryGetKeyboardData(IntPtr pointer, out KeyboardData data)
    {
        data = default;
        if (pointer == IntPtr.Zero)
        {
            return false;
        }

        data = Marshal.PtrToStructure<KeyboardData>(pointer);
        return true;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyboardData
    {
        public uint vkCode;
        public uint scanCode;
        public uint flags;
        public uint time;
        public UIntPtr extraInfo;
    }

    private delegate IntPtr LowLevelKeyboardProc(int code, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr SetWindowsHookEx(
        int idHook,
        LowLevelKeyboardProc lpfn,
        IntPtr hMod,
        uint dwThreadId);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool UnhookWindowsHookEx(IntPtr hhk);

    [DllImport("user32.dll")]
    private static extern IntPtr CallNextHookEx(IntPtr hhk, int nCode, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll")]
    private static extern IntPtr GetKeyboardLayout(uint idThread);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetKeyboardState(byte[] lpKeyState);

    [DllImport("user32.dll")]
    private static extern int ToUnicodeEx(
        uint wVirtKey,
        uint wScanCode,
        byte[] lpKeyState,
        char[] lpBuff,
        int cchBuff,
        uint dwFlags,
        IntPtr hkl);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandle(string? lpModuleName);
}
