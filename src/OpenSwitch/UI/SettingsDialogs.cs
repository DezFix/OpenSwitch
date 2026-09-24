namespace OpenSwitch.UI;

internal sealed class HotkeyCaptureDialog : Form
{
    private readonly Localizer localizer;
    private readonly Label shortcutLabel;

    public HotkeyCaptureDialog(Localizer localizer, string currentShortcut)
    {
        this.localizer = localizer;
        Shortcut = currentShortcut;
        Text = localizer.Get("Hotkeys");
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(390, 150);
        KeyPreview = true;

        var description = new Label
        {
            AutoSize = true,
            Text = localizer.Get("PressShortcut")
        };
        shortcutLabel = new Label
        {
            AutoSize = true,
            Font = new Font("Segoe UI", 14, FontStyle.Bold),
            Text = string.IsNullOrWhiteSpace(currentShortcut) ? localizer.Get("Disabled") : currentShortcut
        };
        var hint = new Label
        {
            AutoSize = true,
            ForeColor = SystemColors.GrayText,
            Text = localizer.Get("Cancel")
        };
        var saveButton = new Button
        {
            AutoSize = true,
            Text = localizer.Get("Save")
        };
        var cancelButton = new Button
        {
            AutoSize = true,
            Text = localizer.Get("Cancel")
        };
        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 42
        };
        saveButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.OK;
            Close();
        };
        cancelButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.Cancel;
            Close();
        };
        buttons.Controls.Add(saveButton);
        buttons.Controls.Add(cancelButton);

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 4,
            Padding = new Padding(12)
        };
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 30));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 50));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 30));
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.Controls.Add(description, 0, 0);
        layout.Controls.Add(shortcutLabel, 0, 1);
        layout.Controls.Add(hint, 0, 2);
        layout.Controls.Add(buttons, 0, 3);
        Controls.Add(layout);
        KeyDown += OnKeyDown;
    }

    public string Shortcut { get; private set; } = string.Empty;

    private void OnKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            DialogResult = DialogResult.Cancel;
            Close();
            return;
        }

        if (e.KeyCode is Keys.ControlKey or Keys.Menu or Keys.ShiftKey or Keys.LWin or Keys.RWin)
        {
            return;
        }

        Shortcut = FormatShortcut(e.Modifiers, e.KeyCode);
        shortcutLabel.Text = Shortcut;
        e.Handled = true;
    }

    private static string FormatShortcut(Keys modifiers, Keys key)
    {
        var parts = new List<string>();
        if (modifiers.HasFlag(Keys.Control))
        {
            parts.Add("Ctrl");
        }

        if (modifiers.HasFlag(Keys.Alt))
        {
            parts.Add("Alt");
        }

        if (modifiers.HasFlag(Keys.Shift))
        {
            parts.Add("Shift");
        }

        if (modifiers.HasFlag(Keys.LWin) || modifiers.HasFlag(Keys.RWin))
        {
            parts.Add("Win");
        }

        parts.Add(key.ToString());
        return string.Join("+", parts);
    }
}

internal sealed class RuleEditorDialog : Form
{
    private readonly Localizer localizer;
    private readonly TextBox nameTextBox;
    private readonly TextBox conditionTextBox;
    private readonly TextBox regexTextBox;
    private readonly ComboBox actionComboBox;
    private readonly CheckBox enabledCheckBox;

    public RuleEditorDialog(Localizer localizer, AutoSwitchRule? source)
    {
        this.localizer = localizer;
        Text = localizer.Get("SwitchRules");
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(500, 300);

        nameTextBox = new TextBox { Dock = DockStyle.Fill };
        conditionTextBox = new TextBox { Dock = DockStyle.Fill };
        regexTextBox = new TextBox { Dock = DockStyle.Fill };
        actionComboBox = new ComboBox
        {
            Dock = DockStyle.Fill,
            DropDownStyle = ComboBoxStyle.DropDownList
        };
        actionComboBox.Items.AddRange(["Switch", "DoNotSwitch", "Convert", "Transliterate"]);
        enabledCheckBox = new CheckBox { AutoSize = true, Text = localizer.Get("Enabled") };

        if (source is not null)
        {
            nameTextBox.Text = source.Name;
            conditionTextBox.Text = source.Condition;
            regexTextBox.Text = source.Regex;
            actionComboBox.SelectedItem = source.Action;
            enabledCheckBox.Checked = source.Enabled;
        }
        else
        {
            actionComboBox.SelectedIndex = 0;
            enabledCheckBox.Checked = true;
        }

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 5,
            Padding = new Padding(12)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 170));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        AddRow(layout, 0, localizer.Get("Name"), nameTextBox);
        AddRow(layout, 1, localizer.Get("Condition"), conditionTextBox);
        AddRow(layout, 2, localizer.Get("Regex"), regexTextBox);
        AddRow(layout, 3, localizer.Get("Action"), actionComboBox);
        layout.Controls.Add(enabledCheckBox, 1, 4);
        Controls.Add(layout);

        var saveButton = new Button { AutoSize = true, Text = localizer.Get("Save") };
        var cancelButton = new Button { AutoSize = true, Text = localizer.Get("Cancel") };
        saveButton.Click += (_, _) => Save();
        cancelButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.Cancel;
            Close();
        };
        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 42
        };
        buttons.Controls.Add(saveButton);
        buttons.Controls.Add(cancelButton);
        Controls.Add(buttons);
    }

    public AutoSwitchRule Rule { get; private set; } = new();

    private void Save()
    {
        if (!string.IsNullOrWhiteSpace(regexTextBox.Text))
        {
            try
            {
                _ = new System.Text.RegularExpressions.Regex(regexTextBox.Text);
            }
            catch (ArgumentException)
            {
                MessageBox.Show(this, localizer.Get("InvalidRegex"), localizer.Get("AppName"), MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
        }

        Rule = new AutoSwitchRule
        {
            Name = nameTextBox.Text.Trim(),
            Condition = conditionTextBox.Text.Trim(),
            Regex = regexTextBox.Text,
            Action = actionComboBox.SelectedItem?.ToString() ?? "Switch",
            Enabled = enabledCheckBox.Checked
        };
        DialogResult = DialogResult.OK;
        Close();
    }

    private static void AddRow(TableLayoutPanel layout, int row, string label, Control control)
    {
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 42));
        layout.Controls.Add(new Label { AutoSize = true, Anchor = AnchorStyles.Left }, 0, row);
        layout.GetControlFromPosition(0, row)!.Text = label;
        layout.Controls.Add(control, 1, row);
    }
}

internal sealed class ProgramRuleDialog : Form
{
    private readonly Localizer localizer;
    private readonly TextBox pathTextBox;
    private readonly TextBox windowTitleTextBox;
    private readonly TextBox processNameTextBox;
    private readonly CheckBox enabledCheckBox;

    public ProgramRuleDialog(Localizer localizer, ProgramRule? source)
    {
        this.localizer = localizer;
        Text = localizer.Get("AddProgram");
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(560, 260);

        pathTextBox = new TextBox { Dock = DockStyle.Fill };
        windowTitleTextBox = new TextBox { Dock = DockStyle.Fill };
        processNameTextBox = new TextBox { Dock = DockStyle.Fill };
        enabledCheckBox = new CheckBox { AutoSize = true, Text = localizer.Get("Enabled") };
        if (source is not null)
        {
            pathTextBox.Text = source.Path;
            windowTitleTextBox.Text = source.WindowTitle;
            processNameTextBox.Text = source.ProcessName;
            enabledCheckBox.Checked = source.Enabled;
        }
        else
        {
            enabledCheckBox.Checked = true;
        }

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 4,
            Padding = new Padding(12)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 170));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var pathHost = new Panel { Dock = DockStyle.Fill, Height = 30 };
        var browseButton = new Button
        {
            AutoSize = false,
            Dock = DockStyle.Right,
            Width = 86,
            Text = localizer.Get("Browse")
        };
        pathTextBox.Dock = DockStyle.Fill;
        pathHost.Controls.Add(pathTextBox);
        pathHost.Controls.Add(browseButton);
        AddRow(layout, 0, localizer.Get("Path"), pathHost);
        AddRow(layout, 1, localizer.Get("WindowTitle"), windowTitleTextBox);
        AddRow(layout, 2, localizer.Get("ProcessName"), processNameTextBox);
        layout.Controls.Add(enabledCheckBox, 1, 3);
        browseButton.Click += (_, _) =>
        {
            using var dialog = new OpenFileDialog
            {
                Filter = "Applications|*.exe;*.lnk|All files|*.*",
                CheckFileExists = true
            };
            if (dialog.ShowDialog(this) == DialogResult.OK)
            {
                pathTextBox.Text = dialog.FileName;
            }
        };
        Controls.Add(layout);

        var saveButton = new Button { AutoSize = true, Text = localizer.Get("Save") };
        var cancelButton = new Button { AutoSize = true, Text = localizer.Get("Cancel") };
        saveButton.Click += (_, _) => Save();
        cancelButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.Cancel;
            Close();
        };
        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 42
        };
        buttons.Controls.Add(saveButton);
        buttons.Controls.Add(cancelButton);
        Controls.Add(buttons);
    }

    public ProgramRule Rule { get; private set; } = new();

    private void Save()
    {
        Rule = new ProgramRule
        {
            Path = pathTextBox.Text.Trim(),
            WindowTitle = windowTitleTextBox.Text.Trim(),
            ProcessName = processNameTextBox.Text.Trim(),
            Enabled = enabledCheckBox.Checked
        };
        DialogResult = DialogResult.OK;
        Close();
    }

    private static void AddRow(TableLayoutPanel layout, int row, string label, Control control)
    {
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 42));
        var labelControl = new Label { AutoSize = true, Anchor = AnchorStyles.Left, Text = label };
        layout.Controls.Add(labelControl, 0, row);
        layout.Controls.Add(control, 1, row);
    }
}

internal sealed class ReplacementEditorDialog : Form
{
    private readonly Localizer localizer;
    private readonly TextBox findTextBox;
    private readonly TextBox replaceTextBox;
    private readonly TextBox fieldTextBox;
    private readonly ComboBox triggerComboBox;
    private readonly CheckBox enabledCheckBox;

    public ReplacementEditorDialog(Localizer localizer, AutoReplacement? source)
    {
        this.localizer = localizer;
        Text = localizer.Get("AutoReplacement");
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(520, 280);

        findTextBox = new TextBox { Dock = DockStyle.Fill };
        replaceTextBox = new TextBox { Dock = DockStyle.Fill };
        fieldTextBox = new TextBox { Dock = DockStyle.Fill };
        triggerComboBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        triggerComboBox.Items.AddRange(["Enter", "Tab"]);
        enabledCheckBox = new CheckBox { AutoSize = true, Text = localizer.Get("Enabled") };
        findTextBox.Text = source?.Find ?? string.Empty;
        replaceTextBox.Text = source?.ReplaceWith ?? string.Empty;
        fieldTextBox.Text = source?.Field ?? string.Empty;
        triggerComboBox.SelectedItem = source?.TriggerKey ?? "Enter";
        enabledCheckBox.Checked = source?.Enabled ?? true;

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 5,
            Padding = new Padding(12)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 180));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        AddRow(layout, 0, localizer.Get("WhatToReplace"), findTextBox);
        AddRow(layout, 1, localizer.Get("ReplaceWith"), replaceTextBox);
        AddRow(layout, 2, localizer.Get("OnlyInField"), fieldTextBox);
        AddRow(layout, 3, localizer.Get("TriggerKey"), triggerComboBox);
        layout.Controls.Add(enabledCheckBox, 1, 4);
        Controls.Add(layout);

        var saveButton = new Button { AutoSize = true, Text = localizer.Get("Save") };
        var cancelButton = new Button { AutoSize = true, Text = localizer.Get("Cancel") };
        saveButton.Click += (_, _) => Save();
        cancelButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.Cancel;
            Close();
        };
        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 42
        };
        buttons.Controls.Add(saveButton);
        buttons.Controls.Add(cancelButton);
        Controls.Add(buttons);
    }

    public AutoReplacement Replacement { get; private set; } = new();

    private void Save()
    {
        Replacement = new AutoReplacement
        {
            Find = findTextBox.Text,
            ReplaceWith = replaceTextBox.Text,
            Field = fieldTextBox.Text,
            TriggerKey = triggerComboBox.SelectedItem?.ToString() ?? "Enter",
            Enabled = enabledCheckBox.Checked
        };
        DialogResult = DialogResult.OK;
        Close();
    }

    private static void AddRow(TableLayoutPanel layout, int row, string label, Control control)
    {
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 42));
        layout.Controls.Add(new Label { AutoSize = true, Anchor = AnchorStyles.Left, Text = label }, 0, row);
        layout.Controls.Add(control, 1, row);
    }
}

internal sealed class SoundEditorDialog : Form
{
    private readonly Localizer localizer;
    private readonly ComboBox eventComboBox;
    private readonly TextBox fileTextBox;
    private readonly CheckBox systemCheckBox;
    private readonly CheckBox enabledCheckBox;

    public SoundEditorDialog(Localizer localizer, SoundEventSettings? source)
    {
        this.localizer = localizer;
        Text = localizer.Get("SoundEvent");
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterParent;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(560, 240);

        eventComboBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        eventComboBox.Items.AddRange(["AutoSwitch", "CapsLock", "AutoReplacement", "Clipboard", "DayNight"]);
        fileTextBox = new TextBox { Dock = DockStyle.Fill };
        systemCheckBox = new CheckBox { AutoSize = true, Text = localizer.Get("UseSystemSounds") };
        enabledCheckBox = new CheckBox { AutoSize = true, Text = localizer.Get("Enabled") };
        eventComboBox.SelectedItem = source?.Event ?? "AutoSwitch";
        fileTextBox.Text = source?.FilePath ?? string.Empty;
        systemCheckBox.Checked = source?.UseSystemSound ?? true;
        enabledCheckBox.Checked = source?.Enabled ?? true;

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 4,
            Padding = new Padding(12)
        };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 220));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var fileHost = new Panel { Dock = DockStyle.Fill, Height = 30 };
        var browseButton = new Button
        {
            AutoSize = false,
            Dock = DockStyle.Right,
            Width = 86,
            Text = localizer.Get("Browse")
        };
        fileTextBox.Dock = DockStyle.Fill;
        fileHost.Controls.Add(fileTextBox);
        fileHost.Controls.Add(browseButton);
        AddRow(layout, 0, localizer.Get("SoundEvent"), eventComboBox);
        AddRow(layout, 1, localizer.Get("SoundFile"), fileHost);
        layout.Controls.Add(systemCheckBox, 1, 2);
        layout.Controls.Add(enabledCheckBox, 1, 3);
        browseButton.Click += (_, _) =>
        {
            using var dialog = new OpenFileDialog
            {
                Filter = "Audio|*.wav;*.mp3;*.ogg|All files|*.*"
            };
            if (dialog.ShowDialog(this) == DialogResult.OK)
            {
                fileTextBox.Text = dialog.FileName;
            }
        };
        Controls.Add(layout);

        var saveButton = new Button { AutoSize = true, Text = localizer.Get("Save") };
        var cancelButton = new Button { AutoSize = true, Text = localizer.Get("Cancel") };
        saveButton.Click += (_, _) => Save();
        cancelButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.Cancel;
            Close();
        };
        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 42
        };
        buttons.Controls.Add(saveButton);
        buttons.Controls.Add(cancelButton);
        Controls.Add(buttons);
    }

    public SoundEventSettings Sound { get; private set; } = new();

    private void Save()
    {
        Sound = new SoundEventSettings
        {
            Event = eventComboBox.SelectedItem?.ToString() ?? "AutoSwitch",
            FilePath = fileTextBox.Text.Trim(),
            UseSystemSound = systemCheckBox.Checked,
            Enabled = enabledCheckBox.Checked
        };
        DialogResult = DialogResult.OK;
        Close();
    }

    private static void AddRow(TableLayoutPanel layout, int row, string label, Control control)
    {
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 42));
        layout.Controls.Add(new Label { AutoSize = true, Anchor = AnchorStyles.Left, Text = label }, 0, row);
        layout.Controls.Add(control, 1, row);
    }
}
