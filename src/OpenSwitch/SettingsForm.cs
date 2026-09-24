using OpenSwitch.UI;

namespace OpenSwitch;

public sealed class SettingsForm : Form
{
    private readonly AppSettings workingSettings;
    private readonly Localizer localizer;
    private readonly Action<AppSettings> saveSettings;
    private readonly TreeView navigation;
    private readonly Panel pageHost;
    private readonly Dictionary<string, Control> pages = [];

    public SettingsForm(
        AppSettings settings,
        Localizer localizer,
        Action<AppSettings> saveSettings,
        Action? openDiary = null,
        Action<string>? testSound = null)
    {
        workingSettings = SettingsStore.Clone(settings);
        this.localizer = localizer;
        this.saveSettings = saveSettings;
        Text = $"{localizer.Get("AppName")} — {localizer.Get("Settings")}";
        StartPosition = FormStartPosition.CenterScreen;
        MinimumSize = new Size(820, 560);
        ClientSize = new Size(960, 650);
        ShowInTaskbar = false;

        navigation = new TreeView
        {
            Dock = DockStyle.Fill,
            HideSelection = false,
            FullRowSelect = true
        };
        pageHost = new Panel
        {
            Dock = DockStyle.Fill
        };

        BuildPages(openDiary, testSound);
        BuildNavigation();

        var split = new SplitContainer
        {
            Dock = DockStyle.Fill,
            Size = new Size(960, 600),
            FixedPanel = FixedPanel.Panel1,
            SplitterDistance = 190,
            Panel1MinSize = 150,
            Panel2MinSize = 500
        };
        split.Panel1.Controls.Add(navigation);
        split.Panel2.Controls.Add(pageHost);

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
        saveButton.Click += (_, _) => SaveAndClose();
        cancelButton.Click += (_, _) =>
        {
            DialogResult = DialogResult.Cancel;
            Close();
        };
        var footer = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.RightToLeft,
            Padding = new Padding(8),
            WrapContents = false
        };
        footer.Controls.Add(saveButton);
        footer.Controls.Add(cancelButton);

        var root = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 2
        };
        root.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        root.RowStyles.Add(new RowStyle(SizeType.Absolute, 50));
        root.Controls.Add(split, 0, 0);
        root.Controls.Add(footer, 0, 1);
        Controls.Add(root);

        Navigate("General");
    }

    private void BuildPages(Action? openDiary, Action<string>? testSound)
    {
        AddPage("General", SettingsPageFactory.CreateGeneral(workingSettings, localizer));
        AddPage("Additional", SettingsPageFactory.CreateAdditional(workingSettings, localizer));
        AddPage("Hotkeys", SettingsPageFactory.CreateHotkeys(workingSettings, localizer));
        AddPage("SwitchRules", SettingsPageFactory.CreateRules(workingSettings, localizer));
        AddPage("ProgramSwitching", SettingsPageFactory.CreateProgramRules(workingSettings, localizer));
        AddPage("Troubleshooting", SettingsPageFactory.CreateTroubleshooting(workingSettings, localizer));
        AddPage("AutoReplacement", SettingsPageFactory.CreateReplacements(workingSettings, localizer));
        AddPage("Sounds", SettingsPageFactory.CreateSounds(workingSettings, localizer, testSound));
        AddPage("Diary", SettingsPageFactory.CreateDiary(workingSettings, localizer, openDiary));
        AddPage("About", SettingsPageFactory.CreateAbout(localizer));
    }

    private void BuildNavigation()
    {
        navigation.Nodes.Clear();
        foreach (var (key, _) in pages)
        {
            navigation.Nodes.Add(new TreeNode(localizer.Get(key)) { Tag = key });
        }

        navigation.AfterSelect += (_, _) =>
        {
            if (navigation.SelectedNode?.Tag is string key)
            {
                Navigate(key);
            }
        };
    }

    private void AddPage(string key, Control page)
    {
        page.Visible = false;
        pages.Add(key, page);
        pageHost.Controls.Add(page);
    }

    private void Navigate(string key)
    {
        foreach (var page in pages.Values)
        {
            page.Visible = false;
        }

        if (pages.TryGetValue(key, out var selectedPage))
        {
            selectedPage.Visible = true;
            selectedPage.BringToFront();
        }

        if (navigation.Nodes.Cast<TreeNode>().FirstOrDefault(node => Equals(node.Tag, key)) is { } node)
        {
            if (!ReferenceEquals(navigation.SelectedNode, node))
            {
                navigation.SelectedNode = node;
            }
        }
    }

    private void SaveAndClose()
    {
        try
        {
            saveSettings(workingSettings);
            DialogResult = DialogResult.OK;
            Close();
        }
        catch (Exception exception)
        {
            MessageBox.Show(
                this,
                exception.Message,
                localizer.Get("Error"),
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning);
        }
    }
}
