using System.Drawing;
using System.Windows.Forms;

namespace OpenSwitch;

public static class AppTheme
{
    public static readonly Color DarkBackground = Color.FromArgb(24, 27, 34);
    public static readonly Color DarkSurface = Color.FromArgb(34, 38, 47);
    public static readonly Color DarkSurfaceAlt = Color.FromArgb(44, 49, 60);
    public static readonly Color DarkText = Color.FromArgb(235, 238, 245);
    public static readonly Color DarkMutedText = Color.FromArgb(164, 172, 188);
    public static readonly Color DarkAccent = Color.FromArgb(76, 145, 230);
    public static readonly Color DarkBorder = Color.FromArgb(70, 78, 94);

    public static void Apply(Form form, bool darkTheme)
    {
        ApplyForm(form, darkTheme);
        ApplyChildren(form, darkTheme);
    }

    public static void Apply(ContextMenuStrip menu, bool darkTheme)
    {
        menu.BackColor = darkTheme ? DarkSurface : SystemColors.Control;
        menu.ForeColor = darkTheme ? DarkText : SystemColors.ControlText;
        menu.RenderMode = darkTheme ? ToolStripRenderMode.Professional : ToolStripRenderMode.System;
        menu.Renderer = darkTheme ? new DarkMenuRenderer() : new ToolStripSystemRenderer();
        foreach (ToolStripItem item in menu.Items)
        {
            ApplyMenuItem(item, darkTheme);
        }
    }

    private static void ApplyForm(Form form, bool darkTheme)
    {
        form.BackColor = darkTheme ? DarkBackground : SystemColors.Control;
        form.ForeColor = darkTheme ? DarkText : SystemColors.ControlText;
    }

    private static void ApplyChildren(Control parent, bool darkTheme)
    {
        if (parent is SplitContainer split)
        {
            ApplyControl(split.Panel1, darkTheme);
            ApplyControl(split.Panel2, darkTheme);
        }

        foreach (Control child in parent.Controls)
        {
            ApplyControl(child, darkTheme);
        }
    }

    private static void ApplyControl(Control control, bool darkTheme)
    {
        if (control is SplitContainer split)
        {
            split.BackColor = darkTheme ? DarkBackground : SystemColors.Control;
            split.ForeColor = darkTheme ? DarkText : SystemColors.ControlText;
            ApplyControl(split.Panel1, darkTheme);
            ApplyControl(split.Panel2, darkTheme);
            return;
        }

        control.BackColor = darkTheme ? GetSurfaceColor(control) : GetLightSurfaceColor(control);
        control.ForeColor = darkTheme ? GetTextColor(control) : SystemColors.ControlText;

        switch (control)
        {
            case TreeView tree:
                tree.BorderStyle = darkTheme ? BorderStyle.None : BorderStyle.FixedSingle;
                break;
            case ListView list:
                list.BorderStyle = darkTheme ? BorderStyle.None : BorderStyle.FixedSingle;
                list.GridLines = !darkTheme;
                break;
            case TextBox textBox:
                textBox.BorderStyle = BorderStyle.FixedSingle;
                break;
            case ComboBox combo:
                combo.FlatStyle = darkTheme ? FlatStyle.Flat : FlatStyle.Standard;
                break;
            case NumericUpDown numeric:
                numeric.BorderStyle = BorderStyle.FixedSingle;
                break;
            case Button button:
                button.FlatStyle = FlatStyle.Flat;
                button.FlatAppearance.BorderColor = darkTheme ? DarkBorder : SystemColors.ControlDark;
                button.FlatAppearance.MouseOverBackColor = darkTheme ? DarkSurfaceAlt : SystemColors.ControlLight;
                button.FlatAppearance.MouseDownBackColor = darkTheme ? DarkAccent : SystemColors.ControlDark;
                break;
            case LinkLabel link:
                link.LinkColor = darkTheme ? DarkAccent : SystemColors.HotTrack;
                break;
        }

        ApplyChildren(control, darkTheme);
    }

    private static Color GetSurfaceColor(Control control)
    {
        return control is TextBox or ComboBox or NumericUpDown or ListView or TreeView
            ? DarkSurface
            : DarkBackground;
    }

    private static Color GetLightSurfaceColor(Control control)
    {
        return control is TextBox or ComboBox or NumericUpDown or ListView
            ? SystemColors.Window
            : SystemColors.Control;
    }

    private static Color GetTextColor(Control control)
    {
        return control is Label or Button
            ? DarkText
            : DarkText;
    }

    private static void ApplyMenuItem(ToolStripItem item, bool darkTheme)
    {
        item.BackColor = darkTheme ? DarkSurface : SystemColors.Control;
        item.ForeColor = darkTheme ? DarkText : SystemColors.ControlText;
        if (item is ToolStripDropDownItem dropDown)
        {
            foreach (ToolStripItem child in dropDown.DropDownItems)
            {
                ApplyMenuItem(child, darkTheme);
            }
        }
    }

    private sealed class DarkColorTable : ProfessionalColorTable
    {
        public override Color MenuItemSelected => Color.FromArgb(57, 105, 170);
        public override Color MenuItemSelectedGradientBegin => Color.FromArgb(57, 105, 170);
        public override Color MenuItemSelectedGradientEnd => Color.FromArgb(57, 105, 170);
        public override Color MenuItemPressedGradientBegin => Color.FromArgb(57, 105, 170);
        public override Color MenuItemPressedGradientEnd => Color.FromArgb(57, 105, 170);
        public override Color MenuBorder => DarkBorder;
        public override Color ToolStripDropDownBackground => DarkSurface;
        public override Color ImageMarginGradientBegin => DarkSurface;
        public override Color ImageMarginGradientMiddle => DarkSurface;
        public override Color ImageMarginGradientEnd => DarkSurface;
        public override Color SeparatorDark => DarkBorder;
        public override Color SeparatorLight => DarkBorder;
        public override Color CheckBackground => DarkSurfaceAlt;
        public override Color CheckSelectedBackground => DarkAccent;
        public override Color ButtonSelectedHighlight => DarkSurfaceAlt;
        public override Color ButtonSelectedBorder => DarkAccent;
    }

    private sealed class DarkMenuRenderer : ToolStripProfessionalRenderer
    {
        public DarkMenuRenderer()
            : base(new DarkColorTable())
        {
            RoundedEdges = false;
        }
    }
}
