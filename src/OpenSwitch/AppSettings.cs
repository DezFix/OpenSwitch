using System.Text.Json;

namespace OpenSwitch;

public sealed class AppSettings
{
    public int Version { get; set; } = 2;
    public string Language { get; set; } = "auto";
    public bool StartWithWindows { get; set; }
    public bool StartAsAdministrator { get; set; }
    public bool CheckForUpdates { get; set; }
    public bool OfferBetaVersions { get; set; }
    public bool ShowFloatingIndicator { get; set; } = true;
    public bool HideIndicatorAfterSwitch { get; set; }
    public bool UseCountryFlags { get; set; }
    public bool FullBrightnessFlags { get; set; }
    public bool IgnoreHotkeysWhenDisabled { get; set; }
    public bool AutoSwitchEnabled { get; set; } = true;
    public bool SwitchByRightControl { get; set; }
    public bool OnlyRussianEnglish { get; set; } = true;
    public bool SingleLayout { get; set; }
    public bool AdditionalShiftSwitch { get; set; }
    public bool FixAbbreviations { get; set; }
    public bool FixDuplicateLetters { get; set; }
    public bool FixCapsLock { get; set; }
    public bool UseCapsLockAsHotkey { get; set; }
    public bool RewindAfterBufferSwap { get; set; }
    public bool HighlightBufferSwap { get; set; }
    public bool SaveBufferHistory { get; set; }
    public bool ShowNotifications { get; set; } = true;
    public bool SaveToDiary { get; set; }
    public bool AskOnDoubleProbek { get; set; }
    public bool DoNotDisturb { get; set; }
    public bool DoNotSwitchAfterNavigation { get; set; } = true;
    public bool DoNotSwitchAfterBackspace { get; set; } = true;
    public bool DoNotSwitchAfterLeftArrow { get; set; } = true;
    public bool DoNotSwitchAfterRightArrow { get; set; } = true;
    public bool DoNotSwitchAfterUpArrow { get; set; } = true;
    public bool DoNotSwitchAfterDownArrow { get; set; } = true;
    public bool DoNotSwitchAfterDelete { get; set; } = true;
    public bool DoNotSwitchAfterLayoutKey { get; set; } = true;
    public bool OnlyRussianEnglishWorkarounds { get; set; } = true;
    public bool DoNotSwitchBetweenRussianEnglish { get; set; }
    public bool DoNotSwitchOnTabEnter { get; set; }
    public bool DoNotInteractWithProgramRules { get; set; }
    public bool KeepDataInPackage { get; set; }
    public bool OnlyExplorerLaunch { get; set; }
    public bool ShowTrayMenu { get; set; } = true;
    public int OfferRuleAfterAttempts { get; set; } = 2;
    public List<HotkeyBinding> Hotkeys { get; set; } = CreateDefaultHotkeys();
    public List<AutoSwitchRule> Rules { get; set; } = [];
    public List<ProgramRule> ProgramRules { get; set; } = [];
    public List<AutoReplacement> Replacements { get; set; } = [];
    public List<SoundEventSettings> SoundEvents { get; set; } = CreateDefaultSoundEvents();
    public DiarySettings Diary { get; set; } = new();

    public static void Normalize(AppSettings settings)
    {
        settings.Version = Math.Max(settings.Version, 2);
        settings.Language ??= "auto";
        settings.Hotkeys ??= CreateDefaultHotkeys();
        settings.Rules ??= [];
        settings.ProgramRules ??= [];
        settings.Replacements ??= [];
        settings.SoundEvents ??= CreateDefaultSoundEvents();
        settings.Diary ??= new DiarySettings();
    }

    private static List<HotkeyBinding> CreateDefaultHotkeys()
    {
        return
        [
            new HotkeyBinding { Action = "CurrentWord", Shortcut = "Scroll" },
            new HotkeyBinding { Action = "SelectedText", Shortcut = "Shift+Scroll" },
            new HotkeyBinding { Action = "ClipboardSelection", Shortcut = "Alt+Break" },
            new HotkeyBinding { Action = "TransliterateSelection", Shortcut = "Alt+Scroll" },
            new HotkeyBinding { Action = "ConvertDigits", Shortcut = "" },
            new HotkeyBinding { Action = "TogglePunctuation", Shortcut = "" },
            new HotkeyBinding { Action = "ToggleLayoutEffects", Shortcut = "" },
            new HotkeyBinding { Action = "InsertWithoutFormatting", Shortcut = "" },
            new HotkeyBinding { Action = "OpenSettings", Shortcut = "" },
            new HotkeyBinding { Action = "ShowClipboardHistory", Shortcut = "" },
            new HotkeyBinding { Action = "SwitchClipboardLayout", Shortcut = "" },
            new HotkeyBinding { Action = "TransliterateClipboard", Shortcut = "" },
            new HotkeyBinding { Action = "SwapClipboard", Shortcut = "" },
            new HotkeyBinding { Action = "ShowAutoReplacementHistory", Shortcut = "" },
            new HotkeyBinding { Action = "AddTextToAutoReplacement", Shortcut = "" },
            new HotkeyBinding { Action = "FillFromClipboardHistory", Shortcut = "" },
            new HotkeyBinding { Action = "SaveClipboardToDiary", Shortcut = "" },
            new HotkeyBinding { Action = "ToggleDayNight", Shortcut = "" },
            new HotkeyBinding { Action = "ShowHideAutoSwitch", Shortcut = "" }
        ];
    }

    private static List<SoundEventSettings> CreateDefaultSoundEvents()
    {
        return
        [
            new SoundEventSettings { Event = "AutoSwitch", UseSystemSound = true },
            new SoundEventSettings { Event = "CapsLock", UseSystemSound = true },
            new SoundEventSettings { Event = "AutoReplacement", UseSystemSound = true },
            new SoundEventSettings { Event = "Clipboard", UseSystemSound = true },
            new SoundEventSettings { Event = "DayNight", UseSystemSound = false }
        ];
    }
}

public sealed class HotkeyBinding
{
    public string Action { get; set; } = string.Empty;
    public string Shortcut { get; set; } = string.Empty;
    public bool Enabled { get; set; } = true;
}

public sealed class AutoSwitchRule
{
    public string Name { get; set; } = string.Empty;
    public string Condition { get; set; } = string.Empty;
    public string Regex { get; set; } = string.Empty;
    public string Action { get; set; } = "Switch";
    public bool Enabled { get; set; } = true;
}

public sealed class ProgramRule
{
    public string Path { get; set; } = string.Empty;
    public string WindowTitle { get; set; } = string.Empty;
    public string ProcessName { get; set; } = string.Empty;
    public bool Enabled { get; set; } = true;
}

public sealed class AutoReplacement
{
    public string Find { get; set; } = string.Empty;
    public string ReplaceWith { get; set; } = string.Empty;
    public string Field { get; set; } = string.Empty;
    public string TriggerKey { get; set; } = "Enter";
    public bool Enabled { get; set; } = true;
}

public sealed class SoundEventSettings
{
    public string Event { get; set; } = string.Empty;
    public string FilePath { get; set; } = string.Empty;
    public bool UseSystemSound { get; set; }
    public bool Enabled { get; set; } = true;
}

public sealed class DiarySettings
{
    public bool Enabled { get; set; }
    public int WordsPerEntry { get; set; } = 2;
    public int DeleteAfterWords { get; set; } = 30;
    public int DeleteAfterDays { get; set; }
    public bool SaveBufferSwaps { get; set; }
    public bool SaveProgramSwaps { get; set; }
    public bool ExcludePrograms { get; set; } = true;
}

public static class SettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNameCaseInsensitive = true
    };

    private static string SettingsPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "OpenSwitch",
        "settings.json");

    public static AppSettings Load()
    {
        try
        {
            if (!File.Exists(SettingsPath))
            {
                return new AppSettings();
            }

            var settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(SettingsPath), JsonOptions)
                ?? new AppSettings();
            AppSettings.Normalize(settings);
            return settings;
        }
        catch
        {
            return new AppSettings();
        }
    }

    public static AppSettings Clone(AppSettings settings)
    {
        AppSettings.Normalize(settings);
        return JsonSerializer.Deserialize<AppSettings>(
            JsonSerializer.Serialize(settings, JsonOptions),
            JsonOptions) ?? new AppSettings();
    }

    public static void Save(AppSettings settings)
    {
        AppSettings.Normalize(settings);
        var directory = Path.GetDirectoryName(SettingsPath)!;
        Directory.CreateDirectory(directory);
        var temporaryPath = SettingsPath + ".tmp";
        File.WriteAllText(temporaryPath, JsonSerializer.Serialize(settings, JsonOptions));
        File.Move(temporaryPath, SettingsPath, true);
    }
}
