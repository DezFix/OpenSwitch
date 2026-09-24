using System.Runtime.InteropServices;
using OpenSwitch.UI;

namespace OpenSwitch;

public sealed class TrayApplicationContext : ApplicationContext
{
    private readonly NotifyIcon notifyIcon;
    private readonly ContextMenuStrip trayMenu;
    private readonly GlobalHotkeyService hotkeyService;
    private readonly TextTransformer textTransformer;
    private readonly AutoLayoutService autoLayoutService;
    private readonly ClipboardHistoryService clipboardHistoryService;
    private readonly DiaryService diaryService;
    private readonly SoundService soundService;
    private readonly AutoStartService autoStartService;
    private readonly Localizer localizer;
    private readonly Icon trayIcon;
    private SettingsForm? settingsForm;
    private ClipboardHistoryForm? clipboardHistoryForm;
    private DiaryForm? diaryForm;
    private AppSettings settings;

    public TrayApplicationContext()
    {
        settings = SettingsStore.Load();
        localizer = new Localizer(Localizer.ParseLanguage(settings.Language));
        autoStartService = new AutoStartService();
        textTransformer = new TextTransformer();
        clipboardHistoryService = new ClipboardHistoryService(settings.SaveBufferHistory);
        diaryService = new DiaryService(settings);
        soundService = new SoundService(settings);
        autoLayoutService = new AutoLayoutService(settings, textTransformer);
        trayIcon = IconProvider.Load();

        try
        {
            autoStartService.SetEnabled(settings.StartWithWindows);
        }
        catch
        {
        }

        hotkeyService = new GlobalHotkeyService(settings.Hotkeys);
        hotkeyService.HotkeyPressed += DispatchHotkey;

        trayMenu = new ContextMenuStrip();
        notifyIcon = new NotifyIcon
        {
            ContextMenuStrip = trayMenu,
            Icon = trayIcon,
            Text = localizer.Get("AppName"),
            Visible = false
        };
        BuildTrayMenu();
        notifyIcon.Visible = true;
        notifyIcon.DoubleClick += (_, _) => ShowSettings();

        if (hotkeyService.RegistrationErrors.Count > 0)
        {
            ShowError(string.Join(Environment.NewLine, hotkeyService.RegistrationErrors));
        }

        if (!string.IsNullOrWhiteSpace(autoLayoutService.LastError))
        {
            ShowError(autoLayoutService.LastError);
        }
    }

    protected override void ExitThreadCore()
    {
        hotkeyService.Dispose();
        autoLayoutService.Dispose();
        clipboardHistoryService.Dispose();
        notifyIcon.Visible = false;
        notifyIcon.Dispose();
        trayIcon.Dispose();
        trayMenu.Dispose();
        settingsForm?.Dispose();
        base.ExitThreadCore();
    }

    private void BuildTrayMenu()
    {
        if (trayMenu is null)
        {
            return;
        }

        trayMenu.Items.Clear();
        trayMenu.Items.Add(localizer.Get("TrayConvertWord"), null, (_, _) => Transform(currentWord: true));
        trayMenu.Items.Add(localizer.Get("TrayConvertSelection"), null, (_, _) => Transform(currentWord: false));
        trayMenu.Items.Add(new ToolStripSeparator());
        var autoSwitchItem = new ToolStripMenuItem(localizer.Get("TrayAutoSwitch"))
        {
            Checked = settings.AutoSwitchEnabled,
            CheckOnClick = true
        };
        autoSwitchItem.CheckedChanged += (_, _) =>
        {
            settings.AutoSwitchEnabled = autoSwitchItem.Checked;
            SaveSettings(settings);
        };
        trayMenu.Items.Add(autoSwitchItem);
        var clipboardItem = new ToolStripMenuItem(localizer.Get("TrayBuffer"));
        clipboardItem.DropDownItems.Add(localizer.Get("ClipboardHistory"), null, (_, _) => ShowClipboardHistory());
        clipboardItem.DropDownItems.Add(localizer.Get("SwapClipboard"), null, (_, _) => SwapClipboard());
        trayMenu.Items.Add(clipboardItem);
        var moreItem = new ToolStripMenuItem(localizer.Get("TrayMore"));
        moreItem.DropDownItems.Add(localizer.Get("TransliterateSelection"), null, (_, _) => Transform(currentWord: false, TextTransformMode.Transliterate));
        moreItem.DropDownItems.Add(localizer.Get("Diary"), null, (_, _) => ShowDiary());
        moreItem.DropDownItems.Add(localizer.Get("Sounds"), null, (_, _) => ShowSettings());
        trayMenu.Items.Add(moreItem);
        trayMenu.Items.Add(new ToolStripSeparator());
        trayMenu.Items.Add(localizer.Get("Settings"), null, (_, _) => ShowSettings());
        trayMenu.Items.Add(new ToolStripSeparator());
        trayMenu.Items.Add(localizer.Get("Exit"), null, (_, _) => ExitThread());
        notifyIcon.Text = localizer.Get("AppName");
    }

    private void ShowSettings()
    {
        if (settingsForm is { IsDisposed: false })
        {
            if (!settingsForm.Visible)
            {
                settingsForm.Show();
            }

            settingsForm.Activate();
            return;
        }

        settingsForm = new SettingsForm(settings, localizer, SaveSettings, ShowDiary, soundService.Play);
        settingsForm.FormClosed += (_, _) => settingsForm = null;
        settingsForm.Show();
        settingsForm.Activate();
    }

    private void SaveSettings(AppSettings updatedSettings)
    {
        settings = updatedSettings;
        SettingsStore.Save(settings);
        autoStartService.SetEnabled(settings.StartWithWindows);
        autoLayoutService.ApplySettings(settings);
        clipboardHistoryService.ApplySettings(settings);
        diaryService.ApplySettings(settings);
        soundService.ApplySettings(settings);
        hotkeyService.Reconfigure(settings.Hotkeys);
        localizer.SetLanguage(Localizer.ParseLanguage(settings.Language));
        BuildTrayMenu();
    }

    private void DispatchHotkey(string action)
    {
        if (!settings.AutoSwitchEnabled && settings.IgnoreHotkeysWhenDisabled)
        {
            return;
        }

        switch (action)
        {
            case "CurrentWord":
                Transform(currentWord: true);
                break;
            case "SelectedText":
                Transform(currentWord: false, TextTransformMode.Layout);
                break;
            case "TransliterateSelection":
                Transform(currentWord: false, TextTransformMode.Transliterate);
                break;
            case "ClipboardSelection":
            case "SwitchClipboardLayout":
                TransformClipboard(TextTransformMode.Layout);
                break;
            case "TransliterateClipboard":
                TransformClipboard(TextTransformMode.Transliterate);
                break;
            case "SwapClipboard":
                SwapClipboard();
                break;
            case "OpenSettings":
                ShowSettings();
                break;
            case "ShowClipboardHistory":
                ShowClipboardHistory();
                break;
            case "ShowDiary":
                ShowDiary();
                break;
            case "ShowHideAutoSwitch":
                settings.AutoSwitchEnabled = !settings.AutoSwitchEnabled;
                SaveSettings(settings);
                break;
            case "FillFromClipboardHistory":
                ShowClipboardHistory();
                break;
            case "SaveClipboardToDiary":
                SaveClipboardToDiary();
                break;
            case "ShowAutoReplacementHistory":
            case "AddTextToAutoReplacement":
            case "ToggleDayNight":
                ShowSettings();
                break;
            default:
                ShowError(localizer.Get("HotkeyUnavailable"));
                break;
        }
    }

    private void Transform(bool currentWord)
    {
        Transform(currentWord, TextTransformMode.Layout);
    }

    private void Transform(bool currentWord, TextTransformMode mode)
    {
        if (mode == TextTransformMode.Layout && currentWord && !hotkeyService.IsCurrentWordAvailable)
        {
            ShowError("Scroll Lock is already used by another application.");
            return;
        }

        if (mode == TextTransformMode.Layout && !currentWord && !hotkeyService.IsSelectionAvailable)
        {
            ShowError("Shift + Scroll Lock is already used by another application.");
            return;
        }

        if (!textTransformer.TryTransform(currentWord, mode, out var error))
        {
            ShowError(error);
            return;
        }

        diaryService.Add(textTransformer.LastSourceText ?? string.Empty);
        soundService.Play("Clipboard");
        ShowIndicator();
    }

    private void SwapClipboard()
    {
        if (!textTransformer.TrySwapClipboard(out var error))
        {
            ShowError(error);
            return;
        }

        diaryService.Add(textTransformer.LastSourceText ?? string.Empty);
        soundService.Play("Clipboard");
        ShowIndicator();
    }

    private void TransformClipboard(TextTransformMode mode)
    {
        if (!textTransformer.TryTransformClipboard(mode, out var error))
        {
            ShowError(error);
            return;
        }

        diaryService.Add(textTransformer.LastSourceText ?? string.Empty);
        soundService.Play("Clipboard");
        ShowIndicator();
    }

    private void ShowClipboardHistory()
    {
        if (clipboardHistoryForm is { IsDisposed: false })
        {
            clipboardHistoryForm.Activate();
            return;
        }

        clipboardHistoryForm = new ClipboardHistoryForm(clipboardHistoryService, localizer);
        clipboardHistoryForm.FormClosed += (_, _) => clipboardHistoryForm = null;
        clipboardHistoryForm.Show();
        clipboardHistoryForm.Activate();
    }

    private void ShowDiary()
    {
        if (diaryForm is { IsDisposed: false })
        {
            diaryForm.Activate();
            return;
        }

        diaryForm = new DiaryForm(diaryService, localizer);
        diaryForm.FormClosed += (_, _) => diaryForm = null;
        diaryForm.Show();
        diaryForm.Activate();
    }

    private void SaveClipboardToDiary()
    {
        try
        {
            if (Clipboard.ContainsText(TextDataFormat.UnicodeText))
            {
                diaryService.Add(Clipboard.GetText(TextDataFormat.UnicodeText));
            }
        }
        catch (ExternalException)
        {
        }
    }

    private void ShowIndicator()
    {
        if (!settings.ShowFloatingIndicator || !settings.ShowNotifications || settings.DoNotDisturb)
        {
            return;
        }

        notifyIcon.ShowBalloonTip(1200, localizer.Get("AppName"), localizer.Get("TextConverted"), ToolTipIcon.Info);
    }

    private void ShowError(string message)
    {
        if (settings.DoNotDisturb || string.IsNullOrWhiteSpace(message))
        {
            return;
        }

        notifyIcon.ShowBalloonTip(4000, localizer.Get("AppName"), message, ToolTipIcon.Warning);
    }
}
