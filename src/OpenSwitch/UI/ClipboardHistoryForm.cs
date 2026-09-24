using System.Runtime.InteropServices;

namespace OpenSwitch.UI;

public sealed class ClipboardHistoryForm : Form
{
    private readonly ClipboardHistoryService historyService;
    private readonly Localizer localizer;
    private readonly ListView listView;

    public ClipboardHistoryForm(ClipboardHistoryService historyService, Localizer localizer)
    {
        this.historyService = historyService;
        this.localizer = localizer;
        Text = localizer.Get("ClipboardHistory");
        StartPosition = FormStartPosition.CenterScreen;
        ClientSize = new Size(650, 480);
        MinimumSize = new Size(500, 350);

        listView = new ListView
        {
            Dock = DockStyle.Fill,
            View = View.Details,
            FullRowSelect = true,
            GridLines = true
        };
        listView.Columns.Add(localizer.Get("Value"), 500);
        listView.Columns.Add("Date", 130);
        listView.DoubleClick += (_, _) => PasteSelected();

        var pasteButton = CreateButton(localizer.Get("Paste"));
        pasteButton.Click += (_, _) => PasteSelected();
        var convertButton = CreateButton(localizer.Get("TrayConvertSelection"));
        convertButton.Click += (_, _) => ConvertSelected();
        var deleteButton = CreateButton(localizer.Get("Delete"));
        deleteButton.Click += (_, _) => DeleteSelected();
        var clearButton = CreateButton(localizer.Get("Clear"));
        clearButton.Click += (_, _) =>
        {
            historyService.Clear();
            RefreshItems();
        };
        var closeButton = CreateButton(localizer.Get("Close"));
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
        buttons.Controls.Add(convertButton);
        buttons.Controls.Add(pasteButton);

        var layout = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 2
        };
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 48));
        layout.Controls.Add(listView, 0, 0);
        layout.Controls.Add(buttons, 0, 1);
        Controls.Add(layout);
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
        foreach (var entry in historyService.Entries)
        {
            var item = new ListViewItem(entry.Text.Replace("\r", " ").Replace("\n", " ")) { Tag = entry };
            item.SubItems.Add(entry.CreatedAt.ToLocalTime().ToString("g"));
            listView.Items.Add(item);
        }
    }

    private ClipboardEntry? GetSelectedEntry()
    {
        return listView.SelectedItems.Count == 0
            ? null
            : (ClipboardEntry)listView.SelectedItems[0].Tag!;
    }

    private void PasteSelected()
    {
        var entry = GetSelectedEntry();
        if (entry is null)
        {
            return;
        }

        try
        {
            Clipboard.SetText(entry.Text);
            Close();
        }
        catch (ExternalException)
        {
        }
    }

    private void ConvertSelected()
    {
        var entry = GetSelectedEntry();
        if (entry is null)
        {
            return;
        }

        try
        {
            var result = LayoutConverter.Convert(entry.Text);
            historyService.Remove(entry);
            Clipboard.SetText(result.Text);
            RefreshItems();
        }
        catch (ExternalException)
        {
        }
    }

    private void DeleteSelected()
    {
        var entry = GetSelectedEntry();
        if (entry is null)
        {
            return;
        }

        historyService.Remove(entry);
        RefreshItems();
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
}
