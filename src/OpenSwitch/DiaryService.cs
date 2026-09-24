using System.Text.Json;

namespace OpenSwitch;

public sealed class DiaryEntry
{
    public string Text { get; set; } = string.Empty;
    public DateTimeOffset CreatedAt { get; set; } = DateTimeOffset.Now;
}

public sealed class DiaryService
{
    private readonly List<DiaryEntry> entries = [];
    private AppSettings settings;

    public DiaryService(AppSettings settings)
    {
        this.settings = settings;
        Load();
    }

    public IReadOnlyList<DiaryEntry> Entries => entries;

    public void ApplySettings(AppSettings updatedSettings)
    {
        settings = updatedSettings;
    }

    public void Add(string text)
    {
        if (!settings.SaveToDiary || !settings.Diary.Enabled || string.IsNullOrWhiteSpace(text))
        {
            return;
        }

        entries.Add(new DiaryEntry { Text = text.Trim() });
        Trim();
        Save();
    }

    public void Clear()
    {
        entries.Clear();
        Save();
    }

    public void Delete(DiaryEntry entry)
    {
        if (entries.Remove(entry))
        {
            Save();
        }
    }

    private static string DiaryPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "OpenSwitch",
        "diary.json");

    private void Load()
    {
        try
        {
            if (!File.Exists(DiaryPath))
            {
                return;
            }

            var loadedEntries = JsonSerializer.Deserialize<List<DiaryEntry>>(File.ReadAllText(DiaryPath));
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
        try
        {
            var directory = Path.GetDirectoryName(DiaryPath)!;
            Directory.CreateDirectory(directory);
            var temporaryPath = DiaryPath + ".tmp";
            File.WriteAllText(
                temporaryPath,
                JsonSerializer.Serialize(entries, new JsonSerializerOptions { WriteIndented = true }));
            File.Move(temporaryPath, DiaryPath, true);
        }
        catch
        {
        }
    }

    private void Trim()
    {
        if (settings.Diary.DeleteAfterWords > 0)
        {
            var maxEntries = Math.Max(1, settings.Diary.DeleteAfterWords);
            if (entries.Count > maxEntries)
            {
                entries.RemoveRange(0, entries.Count - maxEntries);
            }
        }

        if (settings.Diary.DeleteAfterDays > 0)
        {
            var cutoff = DateTimeOffset.Now.AddDays(-settings.Diary.DeleteAfterDays);
            entries.RemoveAll(entry => entry.CreatedAt < cutoff);
        }
    }
}
