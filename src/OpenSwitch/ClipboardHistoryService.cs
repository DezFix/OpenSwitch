using System.Runtime.InteropServices;
using System.Text.Json;
using System.Windows.Forms;

namespace OpenSwitch;

public sealed class ClipboardEntry
{
    public string Text { get; set; } = string.Empty;
    public DateTimeOffset CreatedAt { get; set; } = DateTimeOffset.Now;
}

public sealed class ClipboardHistoryService : NativeWindow, IDisposable
{
    private const int WmClipboardUpdate = 0x031D;
    private readonly List<ClipboardEntry> entries = [];
    private bool saveHistory;
    private bool handleCreated;

    public ClipboardHistoryService(bool saveHistory)
    {
        this.saveHistory = saveHistory;
        Load();
        CreateHandle(new CreateParams
        {
            Caption = "OpenSwitchClipboardWindow"
        });
        handleCreated = true;
        AddClipboardFormatListener(Handle);
    }

    public IReadOnlyList<ClipboardEntry> Entries => entries;

    public void ApplySettings(AppSettings settings)
    {
        saveHistory = settings.SaveBufferHistory;
    }

    protected override void WndProc(ref Message message)
    {
        if (message.Msg == WmClipboardUpdate)
        {
            AddClipboardText();
        }

        base.WndProc(ref message);
    }

    public void Clear()
    {
        entries.Clear();
        Save();
    }

    public void Remove(ClipboardEntry entry)
    {
        if (entries.Remove(entry))
        {
            Save();
        }
    }

    public void Dispose()
    {
        if (handleCreated)
        {
            RemoveClipboardFormatListener(Handle);
            DestroyHandle();
            handleCreated = false;
        }

        GC.SuppressFinalize(this);
    }

    private void AddClipboardText()
    {
        try
        {
            if (!Clipboard.ContainsText(TextDataFormat.UnicodeText))
            {
                return;
            }

            var text = Clipboard.GetText(TextDataFormat.UnicodeText);
            if (string.IsNullOrWhiteSpace(text) || entries.Any(entry => entry.Text == text))
            {
                return;
            }

            entries.Insert(0, new ClipboardEntry { Text = text });
            if (entries.Count > 100)
            {
                entries.RemoveRange(100, entries.Count - 100);
            }

            Save();
        }
        catch (ExternalException)
        {
        }
    }

    private static string HistoryPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "OpenSwitch",
        "clipboard.json");

    private void Load()
    {
        try
        {
            if (!File.Exists(HistoryPath))
            {
                return;
            }

            var loadedEntries = JsonSerializer.Deserialize<List<ClipboardEntry>>(File.ReadAllText(HistoryPath));
            if (loadedEntries is not null)
            {
                entries.AddRange(loadedEntries.Where(entry => !string.IsNullOrWhiteSpace(entry.Text)));
            }
        }
        catch
        {
        }
    }

    private void Save()
    {
        if (!saveHistory)
        {
            return;
        }

        try
        {
            var directory = Path.GetDirectoryName(HistoryPath)!;
            Directory.CreateDirectory(directory);
            var temporaryPath = HistoryPath + ".tmp";
            File.WriteAllText(
                temporaryPath,
                JsonSerializer.Serialize(entries, new JsonSerializerOptions { WriteIndented = true }));
            File.Move(temporaryPath, HistoryPath, true);
        }
        catch
        {
        }
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool AddClipboardFormatListener(IntPtr hwnd);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool RemoveClipboardFormatListener(IntPtr hwnd);
}
