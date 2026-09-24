namespace OpenSwitch.UI;

internal static class SettingsPageFactory
{
    public static Control CreateGeneral(
        AppSettings settings,
        Localizer localizer,
        Action<bool>? themeChanged = null)
    {
        var (page, content) = CreatePage(localizer.Get("General"), localizer.Get("GeneralDescription"));
        AddCheck(content, localizer.Get("StartWithWindows"), settings.StartWithWindows, value => settings.StartWithWindows = value);
        AddCheck(content, localizer.Get("StartAsAdministrator"), settings.StartAsAdministrator, value => settings.StartAsAdministrator = value);
        AddCheck(content, localizer.Get("CheckForUpdates"), settings.CheckForUpdates, value => settings.CheckForUpdates = value);
        AddCheck(content, localizer.Get("OfferBetaVersions"), settings.OfferBetaVersions, value => settings.OfferBetaVersions = value);
        AddCheck(content, localizer.Get("ShowFloatingIndicator"), settings.ShowFloatingIndicator, value => settings.ShowFloatingIndicator = value);
        AddCheck(content, localizer.Get("HideIndicatorAfterSwitch"), settings.HideIndicatorAfterSwitch, value => settings.HideIndicatorAfterSwitch = value);
        AddCheck(content, localizer.Get("UseCountryFlags"), settings.UseCountryFlags, value => settings.UseCountryFlags = value);
        AddCheck(content, localizer.Get("FullBrightnessFlags"), settings.FullBrightnessFlags, value => settings.FullBrightnessFlags = value);
        AddCheck(content, localizer.Get("AutoSwitch"), settings.AutoSwitchEnabled, value => settings.AutoSwitchEnabled = value);
        AddCheck(content, localizer.Get("IgnoreHotkeysWhenDisabled"), settings.IgnoreHotkeysWhenDisabled, value => settings.IgnoreHotkeysWhenDisabled = value);
        AddCheck(content, localizer.Get("SwitchByRightControl"), settings.SwitchByRightControl, value => settings.SwitchByRightControl = value);
        AddCheck(content, localizer.Get("OnlyRussianEnglish"), settings.OnlyRussianEnglish, value => settings.OnlyRussianEnglish = value);
        AddCheck(content, localizer.Get("SingleLayout"), settings.SingleLayout, value => settings.SingleLayout = value);
        AddCheck(content, localizer.Get("AdditionalShiftSwitch"), settings.AdditionalShiftSwitch, value => settings.AdditionalShiftSwitch = value);
        AddCheck(
            content,
            localizer.Get("DarkTheme"),
            settings.UseDarkTheme,
            value =>
            {
                settings.UseDarkTheme = value;
                themeChanged?.Invoke(value);
            });

        var languageComboBox = new ComboBox
        {
            DropDownStyle = ComboBoxStyle.DropDownList,
            Width = 220
        };
        foreach (var option in localizer.AvailableLanguages)
        {
            var displayName = option.Value switch
            {
                AppLanguage.Auto => localizer.Get("Auto"),
                AppLanguage.English => localizer.Get("English"),
                AppLanguage.Russian => localizer.Get("Russian"),
                AppLanguage.Ukrainian => localizer.Get("Ukrainian"),
                _ => option.DisplayName
            };
            languageComboBox.Items.Add(new LanguageOption(option.Value, option.Code, displayName));
        }
        var currentLanguage = Localizer.ParseLanguage(settings.Language);
        languageComboBox.SelectedItem = languageComboBox.Items
            .Cast<LanguageOption>()
            .First(option => option.Value == currentLanguage);
        languageComboBox.SelectedIndexChanged += (_, _) =>
        {
            if (languageComboBox.SelectedItem is LanguageOption option)
            {
                settings.Language = Localizer.ToStorageValue(option.Value);
            }
        };
        AddRow(content, localizer.Get("Language"), languageComboBox);
        return page;
    }

    public static Control CreateAdditional(AppSettings settings, Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("Additional"), localizer.Get("AdditionalDescription"));
        AddCheck(content, localizer.Get("FixAbbreviations"), settings.FixAbbreviations, value => settings.FixAbbreviations = value);
        AddCheck(content, localizer.Get("FixDuplicateLetters"), settings.FixDuplicateLetters, value => settings.FixDuplicateLetters = value);
        AddCheck(content, localizer.Get("FixCapsLock"), settings.FixCapsLock, value => settings.FixCapsLock = value);
        AddCheck(content, localizer.Get("UseCapsLockAsHotkey"), settings.UseCapsLockAsHotkey, value => settings.UseCapsLockAsHotkey = value);
        AddCheck(content, localizer.Get("RewindAfterBufferSwap"), settings.RewindAfterBufferSwap, value => settings.RewindAfterBufferSwap = value);
        AddCheck(content, localizer.Get("HighlightBufferSwap"), settings.HighlightBufferSwap, value => settings.HighlightBufferSwap = value);
        AddCheck(content, localizer.Get("SaveBufferHistory"), settings.SaveBufferHistory, value => settings.SaveBufferHistory = value);
        AddCheck(content, localizer.Get("ShowNotifications"), settings.ShowNotifications, value => settings.ShowNotifications = value);
        AddCheck(content, localizer.Get("SaveToDiary"), settings.SaveToDiary, value => settings.SaveToDiary = value);
        AddCheck(content, localizer.Get("AskOnDoubleProbek"), settings.AskOnDoubleProbek, value => settings.AskOnDoubleProbek = value);
        AddCheck(content, localizer.Get("DoNotDisturb"), settings.DoNotDisturb, value => settings.DoNotDisturb = value);
        return page;
    }

    public static Control CreateHotkeys(AppSettings settings, Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("Hotkeys"), localizer.Get("HotkeysDescription"));
        var list = CreateListView(localizer.Get("Action"), localizer.Get("Key"));
        list.CheckBoxes = true;
        list.Height = 300;
        var refreshing = false;
        list.ItemCheck += (_, e) =>
        {
            if (!refreshing && e.Index >= 0 && e.Index < settings.Hotkeys.Count)
            {
                settings.Hotkeys[e.Index].Enabled = e.NewValue == CheckState.Checked;
            }
        };

        void Refresh()
        {
            refreshing = true;
            list.Items.Clear();
            foreach (var binding in settings.Hotkeys)
            {
                var item = new ListViewItem(localizer.Get(binding.Action)) { Tag = binding, Checked = binding.Enabled };
                item.SubItems.Add(binding.Shortcut);
                list.Items.Add(item);
            }
            refreshing = false;
        }

        var assignButton = CreateButton(localizer.Get("Edit"));
        assignButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var binding = (HotkeyBinding)list.SelectedItems[0].Tag!;
            using var dialog = new HotkeyCaptureDialog(localizer, binding.Shortcut);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                binding.Shortcut = dialog.Shortcut;
                Refresh();
            }
        };
        var clearButton = CreateButton(localizer.Get("Clear"));
        clearButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var binding = (HotkeyBinding)list.SelectedItems[0].Tag!;
            binding.Shortcut = string.Empty;
            Refresh();
        };
        content.Controls.Add(list);
        content.Controls.Add(CreateButtonRow(assignButton, clearButton));
        Refresh();
        return page;
    }

    public static Control CreateRules(AppSettings settings, Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("SwitchRules"), localizer.Get("RulesDescription"));
        var list = CreateListView(localizer.Get("Name"), localizer.Get("Condition"), localizer.Get("Regex"), localizer.Get("Action"));
        list.CheckBoxes = true;
        list.Height = 300;
        var refreshing = false;
        list.ItemCheck += (_, e) =>
        {
            if (!refreshing && e.Index >= 0 && e.Index < settings.Rules.Count)
            {
                settings.Rules[e.Index].Enabled = e.NewValue == CheckState.Checked;
            }
        };

        void Refresh()
        {
            refreshing = true;
            list.Items.Clear();
            foreach (var rule in settings.Rules)
            {
                var item = new ListViewItem(rule.Name) { Tag = rule, Checked = rule.Enabled };
                item.SubItems.Add(rule.Condition);
                item.SubItems.Add(rule.Regex);
                item.SubItems.Add(rule.Action);
                list.Items.Add(item);
            }
            refreshing = false;
        }

        var addButton = CreateButton(localizer.Get("Add"));
        addButton.Click += (_, _) =>
        {
            using var dialog = new RuleEditorDialog(localizer, null);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                settings.Rules.Add(dialog.Rule);
                Refresh();
            }
        };
        var editButton = CreateButton(localizer.Get("Edit"));
        editButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var rule = (AutoSwitchRule)list.SelectedItems[0].Tag!;
            using var dialog = new RuleEditorDialog(localizer, rule);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                var index = settings.Rules.IndexOf(rule);
                if (index >= 0)
                {
                    settings.Rules[index] = dialog.Rule;
                }
                Refresh();
            }
        };
        var deleteButton = CreateButton(localizer.Get("Delete"));
        deleteButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var rule = (AutoSwitchRule)list.SelectedItems[0].Tag!;
            settings.Rules.Remove(rule);
            Refresh();
        };
        content.Controls.Add(list);
        content.Controls.Add(CreateButtonRow(addButton, editButton, deleteButton));
        var attempts = new NumericUpDown { Minimum = 1, Maximum = 100, Value = Math.Clamp(settings.OfferRuleAfterAttempts, 1, 100), Width = 80 };
        attempts.ValueChanged += (_, _) => settings.OfferRuleAfterAttempts = (int)attempts.Value;
        AddRow(content, localizer.Get("OfferRuleAfterAttempts"), attempts);
        Refresh();
        return page;
    }

    public static Control CreateProgramRules(AppSettings settings, Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("ProgramSwitching"), localizer.Get("ProgramSwitching"));
        var list = CreateListView(localizer.Get("Path"), localizer.Get("WindowTitle"), localizer.Get("ProcessName"), localizer.Get("Enabled"));
        list.CheckBoxes = true;
        list.Height = 320;
        var refreshing = false;
        list.ItemCheck += (_, e) =>
        {
            if (!refreshing && e.Index >= 0 && e.Index < settings.ProgramRules.Count)
            {
                settings.ProgramRules[e.Index].Enabled = e.NewValue == CheckState.Checked;
            }
        };

        void Refresh()
        {
            refreshing = true;
            list.Items.Clear();
            foreach (var program in settings.ProgramRules)
            {
                var item = new ListViewItem(program.Path) { Tag = program, Checked = program.Enabled };
                item.SubItems.Add(program.WindowTitle);
                item.SubItems.Add(program.ProcessName);
                item.SubItems.Add(string.Empty);
                list.Items.Add(item);
            }
            refreshing = false;
        }

        var addButton = CreateButton(localizer.Get("Add"));
        addButton.Click += (_, _) =>
        {
            using var dialog = new ProgramRuleDialog(localizer, null);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                settings.ProgramRules.Add(dialog.Rule);
                Refresh();
            }
        };
        var editButton = CreateButton(localizer.Get("Edit"));
        editButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var program = (ProgramRule)list.SelectedItems[0].Tag!;
            using var dialog = new ProgramRuleDialog(localizer, program);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                var index = settings.ProgramRules.IndexOf(program);
                if (index >= 0)
                {
                    settings.ProgramRules[index] = dialog.Rule;
                }
                Refresh();
            }
        };
        var deleteButton = CreateButton(localizer.Get("Delete"));
        deleteButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var program = (ProgramRule)list.SelectedItems[0].Tag!;
            settings.ProgramRules.Remove(program);
            Refresh();
        };
        content.Controls.Add(list);
        content.Controls.Add(CreateButtonRow(addButton, editButton, deleteButton));
        Refresh();
        return page;
    }

    public static Control CreateTroubleshooting(AppSettings settings, Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("Troubleshooting"), localizer.Get("Troubleshooting"));
        AddCheck(content, localizer.Get("DoNotSwitchWhenStartedWith"), settings.DoNotSwitchAfterNavigation, value => settings.DoNotSwitchAfterNavigation = value);
        AddCheck(content, localizer.Get("Backspace"), settings.DoNotSwitchAfterBackspace, value => settings.DoNotSwitchAfterBackspace = value);
        AddCheck(content, localizer.Get("LeftArrow"), settings.DoNotSwitchAfterLeftArrow, value => settings.DoNotSwitchAfterLeftArrow = value);
        AddCheck(content, localizer.Get("RightArrow"), settings.DoNotSwitchAfterRightArrow, value => settings.DoNotSwitchAfterRightArrow = value);
        AddCheck(content, localizer.Get("UpArrow"), settings.DoNotSwitchAfterUpArrow, value => settings.DoNotSwitchAfterUpArrow = value);
        AddCheck(content, localizer.Get("DownArrow"), settings.DoNotSwitchAfterDownArrow, value => settings.DoNotSwitchAfterDownArrow = value);
        AddCheck(content, localizer.Get("Delete"), settings.DoNotSwitchAfterDelete, value => settings.DoNotSwitchAfterDelete = value);
        AddCheck(content, localizer.Get("LayoutKey"), settings.DoNotSwitchAfterLayoutKey, value => settings.DoNotSwitchAfterLayoutKey = value);
        AddCheck(content, localizer.Get("OnlyRuEnWorkarounds"), settings.OnlyRussianEnglishWorkarounds, value => settings.OnlyRussianEnglishWorkarounds = value);
        AddCheck(content, localizer.Get("DoNotSwitchRuEn"), settings.DoNotSwitchBetweenRussianEnglish, value => settings.DoNotSwitchBetweenRussianEnglish = value);
        AddCheck(content, localizer.Get("DoNotSwitchTabEnter"), settings.DoNotSwitchOnTabEnter, value => settings.DoNotSwitchOnTabEnter = value);
        AddCheck(content, localizer.Get("DoNotInteractWithPrograms"), settings.DoNotInteractWithProgramRules, value => settings.DoNotInteractWithProgramRules = value);
        AddCheck(content, localizer.Get("KeepDataInPackage"), settings.KeepDataInPackage, value => settings.KeepDataInPackage = value);
        AddCheck(content, localizer.Get("OnlyExplorerLaunch"), settings.OnlyExplorerLaunch, value => settings.OnlyExplorerLaunch = value);
        return page;
    }

    public static Control CreateReplacements(AppSettings settings, Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("AutoReplacement"), localizer.Get("AutoReplacement"));
        var list = CreateListView(localizer.Get("WhatToReplace"), localizer.Get("ReplaceWith"), localizer.Get("OnlyInField"), localizer.Get("TriggerKey"));
        list.CheckBoxes = true;
        list.Height = 320;
        var refreshing = false;
        list.ItemCheck += (_, e) =>
        {
            if (!refreshing && e.Index >= 0 && e.Index < settings.Replacements.Count)
            {
                settings.Replacements[e.Index].Enabled = e.NewValue == CheckState.Checked;
            }
        };

        void Refresh()
        {
            refreshing = true;
            list.Items.Clear();
            foreach (var replacement in settings.Replacements)
            {
                var item = new ListViewItem(replacement.Find) { Tag = replacement, Checked = replacement.Enabled };
                item.SubItems.Add(replacement.ReplaceWith);
                item.SubItems.Add(replacement.Field);
                item.SubItems.Add(replacement.TriggerKey);
                list.Items.Add(item);
            }
            refreshing = false;
        }

        var addButton = CreateButton(localizer.Get("Add"));
        addButton.Click += (_, _) =>
        {
            using var dialog = new ReplacementEditorDialog(localizer, null);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                settings.Replacements.Add(dialog.Replacement);
                Refresh();
            }
        };
        var editButton = CreateButton(localizer.Get("Edit"));
        editButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var replacement = (AutoReplacement)list.SelectedItems[0].Tag!;
            using var dialog = new ReplacementEditorDialog(localizer, replacement);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                var index = settings.Replacements.IndexOf(replacement);
                if (index >= 0)
                {
                    settings.Replacements[index] = dialog.Replacement;
                }
                Refresh();
            }
        };
        var deleteButton = CreateButton(localizer.Get("Delete"));
        deleteButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var replacement = (AutoReplacement)list.SelectedItems[0].Tag!;
            settings.Replacements.Remove(replacement);
            Refresh();
        };
        content.Controls.Add(list);
        content.Controls.Add(CreateButtonRow(addButton, editButton, deleteButton));
        Refresh();
        return page;
    }

    public static Control CreateSounds(
        AppSettings settings,
        Localizer localizer,
        Action<string>? testSound = null)
    {
        var (page, content) = CreatePage(localizer.Get("Sounds"), localizer.Get("UseSystemSounds"));
        var list = CreateListView(localizer.Get("SoundEvent"), localizer.Get("SoundFile"), localizer.Get("UseSystemSounds"), localizer.Get("Enabled"));
        list.CheckBoxes = true;
        list.Height = 320;
        var refreshing = false;
        list.ItemCheck += (_, e) =>
        {
            if (!refreshing && e.Index >= 0 && e.Index < settings.SoundEvents.Count)
            {
                settings.SoundEvents[e.Index].Enabled = e.NewValue == CheckState.Checked;
            }
        };

        void Refresh()
        {
            refreshing = true;
            list.Items.Clear();
            foreach (var sound in settings.SoundEvents)
            {
                var item = new ListViewItem(sound.Event) { Tag = sound, Checked = sound.Enabled };
                item.SubItems.Add(sound.FilePath);
                item.SubItems.Add(sound.UseSystemSound ? localizer.Get("Yes") : localizer.Get("No"));
                item.SubItems.Add(string.Empty);
                list.Items.Add(item);
            }
            refreshing = false;
        }

        var addButton = CreateButton(localizer.Get("Add"));
        addButton.Click += (_, _) =>
        {
            using var dialog = new SoundEditorDialog(localizer, null);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                settings.SoundEvents.Add(dialog.Sound);
                Refresh();
            }
        };
        var editButton = CreateButton(localizer.Get("Edit"));
        editButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var sound = (SoundEventSettings)list.SelectedItems[0].Tag!;
            using var dialog = new SoundEditorDialog(localizer, sound);
            if (dialog.ShowDialog(page) == DialogResult.OK)
            {
                var index = settings.SoundEvents.IndexOf(sound);
                if (index >= 0)
                {
                    settings.SoundEvents[index] = dialog.Sound;
                }
                Refresh();
            }
        };
        var deleteButton = CreateButton(localizer.Get("Delete"));
        deleteButton.Click += (_, _) =>
        {
            if (list.SelectedItems.Count == 0)
            {
                return;
            }

            var sound = (SoundEventSettings)list.SelectedItems[0].Tag!;
            settings.SoundEvents.Remove(sound);
            Refresh();
        };
        var buttons = new List<Button> { addButton, editButton, deleteButton };
        if (testSound is not null)
        {
            var testButton = CreateButton(localizer.Get("Test"));
            testButton.Click += (_, _) =>
            {
                if (list.SelectedItems.Count > 0)
                {
                    testSound(((SoundEventSettings)list.SelectedItems[0].Tag!).Event);
                }
            };
            buttons.Add(testButton);
        }
        content.Controls.Add(list);
        content.Controls.Add(CreateButtonRow(buttons.ToArray()));
        Refresh();
        return page;
    }

    public static Control CreateDiary(
        AppSettings settings,
        Localizer localizer,
        Action? openDiary = null)
    {
        var (page, content) = CreatePage(localizer.Get("Diary"), localizer.Get("Diary"));
        var diary = settings.Diary;
        AddCheck(content, localizer.Get("EnableDiary"), diary.Enabled, value => diary.Enabled = value);
        AddCheck(content, localizer.Get("SaveBufferSwaps"), diary.SaveBufferSwaps, value => diary.SaveBufferSwaps = value);
        AddCheck(content, localizer.Get("SaveProgramSwaps"), diary.SaveProgramSwaps, value => diary.SaveProgramSwaps = value);
        AddCheck(content, localizer.Get("DoNotInteractWithPrograms"), diary.ExcludePrograms, value => diary.ExcludePrograms = value);
        AddNumberRow(content, localizer.Get("WordsPerEntry"), diary.WordsPerEntry, 1, 100, value => diary.WordsPerEntry = value);
        AddNumberRow(content, localizer.Get("DeleteAfterWords"), diary.DeleteAfterWords, 0, 10000, value => diary.DeleteAfterWords = value);
        AddNumberRow(content, localizer.Get("DeleteAfterDays"), diary.DeleteAfterDays, 0, 3650, value => diary.DeleteAfterDays = value);
        if (openDiary is not null)
        {
            var openButton = CreateButton(localizer.Get("OpenDiary"));
            openButton.Click += (_, _) => openDiary();
            content.Controls.Add(CreateButtonRow(openButton));
        }
        return page;
    }

    public static Control CreateAbout(Localizer localizer)
    {
        var (page, content) = CreatePage(localizer.Get("About"), localizer.Get("AboutDescription"));
        var name = new Label
        {
            AutoSize = true,
            Font = new Font("Segoe UI", 18, FontStyle.Bold),
            Text = "OpenSwitch"
        };
        var description = new Label
        {
            AutoSize = true,
            Text = localizer.Get("AboutDescription")
        };
        var version = new Label
        {
            AutoSize = true,
            Text = $"{localizer.Get("Version")}: {Application.ProductVersion ?? "1.0.0"}"
        };
        content.Controls.Add(name);
        content.Controls.Add(description);
        content.Controls.Add(version);
        return page;
    }

    private static (Panel Page, FlowLayoutPanel Content) CreatePage(string title, string description)
    {
        var content = new SettingsContentPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.TopDown,
            WrapContents = false,
            AutoScroll = true,
            Padding = new Padding(14)
        };
        var page = new Panel
        {
            Dock = DockStyle.Fill,
            AutoScroll = true
        };
        var titleLabel = new Label
        {
            AutoSize = true,
            Font = new Font("Segoe UI", 14, FontStyle.Bold),
            Text = title
        };
        var descriptionLabel = new Label
        {
            AutoSize = true,
            ForeColor = SystemColors.GrayText,
            Text = description
        };
        content.Controls.Add(titleLabel);
        content.Controls.Add(descriptionLabel);
        page.Controls.Add(content);
        return (page, content);
    }

    private static void AddCheck(FlowLayoutPanel content, string text, bool value, Action<bool> setter)
    {
        var checkBox = new CheckBox
        {
            AutoSize = true,
            Checked = value,
            Text = text,
            Margin = new Padding(0, 4, 0, 4)
        };
        checkBox.CheckedChanged += (_, _) => setter(checkBox.Checked);
        content.Controls.Add(checkBox);
    }

    private static void AddRow(FlowLayoutPanel content, string label, Control control)
    {
        var row = new TableLayoutPanel
        {
            AutoSize = true,
            ColumnCount = 2,
            Dock = DockStyle.Top,
            Margin = new Padding(0, 4, 0, 4)
        };
        row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 300));
        row.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        row.Controls.Add(new Label { AutoSize = true, Anchor = AnchorStyles.Left, Text = label }, 0, 0);
        row.Controls.Add(control, 1, 0);
        content.Controls.Add(row);
    }

    private static void AddNumberRow(
        FlowLayoutPanel content,
        string label,
        int value,
        int minimum,
        int maximum,
        Action<int> setter)
    {
        var numeric = new NumericUpDown
        {
            Minimum = minimum,
            Maximum = maximum,
            Value = Math.Clamp(value, minimum, maximum),
            Width = 100
        };
        numeric.ValueChanged += (_, _) => setter((int)numeric.Value);
        AddRow(content, label, numeric);
    }

    private static ListView CreateListView(params string[] columns)
    {
        var list = new ListView
        {
            View = View.Details,
            FullRowSelect = true,
            GridLines = true,
            Dock = DockStyle.Top,
            HideSelection = false
        };
        foreach (var column in columns)
        {
            list.Columns.Add(column, column == columns[0] ? 180 : 150);
        }
        return list;
    }

    private static Button CreateButton(string text)
    {
        return new Button
        {
            AutoSize = true,
            Text = text,
            Margin = new Padding(0, 0, 6, 0)
        };
    }

    private static FlowLayoutPanel CreateButtonRow(params Button[] buttons)
    {
        var row = new FlowLayoutPanel
        {
            AutoSize = true,
            Dock = DockStyle.Top,
            FlowDirection = FlowDirection.LeftToRight,
            WrapContents = false,
            Margin = new Padding(0, 8, 0, 8)
        };
        row.Controls.AddRange(buttons);
        return row;
    }

    private static void ResizeListColumns(ListView list)
    {
        if (list.Columns.Count == 0)
        {
            return;
        }

        var width = Math.Max(240, list.ClientSize.Width - 4);
        var firstWidth = Math.Max(140, (int)(width * 0.4));
        list.Columns[0].Width = firstWidth;
        var remainingWidth = Math.Max(80, width - firstWidth);
        var otherWidth = Math.Max(80, remainingWidth / Math.Max(1, list.Columns.Count - 1));
        for (var index = 1; index < list.Columns.Count; index++)
        {
            list.Columns[index].Width = otherWidth;
        }
    }

    private sealed class SettingsContentPanel : FlowLayoutPanel
    {
        protected override void OnLayout(LayoutEventArgs layoutEvent)
        {
            base.OnLayout(layoutEvent);
            var width = Math.Max(260, ClientSize.Width - Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - 4);
            foreach (Control child in Controls)
            {
                if (child is ListView or TableLayoutPanel or FlowLayoutPanel)
                {
                    child.Width = width;
                }

                if (child is ListView list)
                {
                    ResizeListColumns(list);
                }
            }
        }
    }
}
