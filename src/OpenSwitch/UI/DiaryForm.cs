namespace OpenSwitch.UI;

public sealed class DiaryForm : Form
{
    private readonly DiaryService diaryService;
    private readonly Localizer localizer;
    private readonly ListView listView;
    private readonly TextBox detailTextBox;

    public DiaryForm(DiaryService diaryService, Localizer localizer)
    {
        this.diaryService = diaryService;
        this.localizer = localizer;
        Text = localizer.Get("Diary");
        StartPosition = FormStartPosition.CenterScreen;
        ClientSize = new Size(760, 520);

        listView = new ListView
        {
            Dock = DockStyle.Fill,
            View = View.Details,
            FullRowSelect = true,
            GridLines = true
        };
        listView.Columns.Add(localizer.Get("Value"), 520);
        listView.Columns.Add("Date", 150);
        listView.SelectedIndexChanged += (_, _) => ShowSelected();

        detailTextBox = new TextBox
        {
            Dock = DockStyle.Fill,
            Multiline = true,
            ReadOnly = true,
            ScrollBars = ScrollBars.Vertical
        };

        var deleteButton = new Button { AutoSize = true, Text = localizer.Get("Delete") };
        var clearButton = new Button { AutoSize = true, Text = localizer.Get("Clear") };
        var closeButton = new Button { AutoSize = true, Text = localizer.Get("Close") };
        deleteButton.Click += (_, _) => DeleteSelected();
        clearButton.Click += (_, _) =>
        {
            diaryService.Clear();
            RefreshItems();
        };
        closeButton.Click += (_, _) => Close();

        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            FlowDirection = FlowDirection.RightToLeft,
            Height = 46,
            Padding = new Padding(6)
        };
        buttons.Controls.Add(closeButton);
        buttons.Controls.Add(clearButton);
        buttons.Controls.Add(deleteButton);

        var split = new SplitContainer
        {
            Dock = DockStyle.Fill,
            Size = new Size(760, 470),
            Orientation = Orientation.Horizontal,
            SplitterDistance = 290
        };
        split.Panel1.Controls.Add(listView);
        split.Panel2.Controls.Add(detailTextBox);
        Controls.Add(split);
        Controls.Add(buttons);
        RefreshItems();
    }

    protected override void OnShown(EventArgs e)
    {
        RefreshItems();
        base.OnShown(e);
    }

    private void RefreshItems()
    {
        listView.Items.Clear();
        foreach (var entry in diaryService.Entries.OrderByDescending(entry => entry.CreatedAt))
        {
            var item = new ListViewItem(entry.Text.Replace("\r", " ").Replace("\n", " ")) { Tag = entry };
            item.SubItems.Add(entry.CreatedAt.ToLocalTime().ToString("g"));
            listView.Items.Add(item);
        }
    }

    private DiaryEntry? SelectedEntry => listView.SelectedItems.Count == 0
        ? null
        : (DiaryEntry)listView.SelectedItems[0].Tag!;

    private void ShowSelected()
    {
        detailTextBox.Text = SelectedEntry?.Text ?? string.Empty;
    }

    private void DeleteSelected()
    {
        var entry = SelectedEntry;
        if (entry is null)
        {
            return;
        }

        diaryService.Delete(entry);
        RefreshItems();
    }
}
