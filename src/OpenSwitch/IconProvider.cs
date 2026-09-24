using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace OpenSwitch;

public static class IconProvider
{
    public static Icon Load()
    {
        var assetDirectory = Path.Combine(AppContext.BaseDirectory, "Assets");
        var iconPath = Path.Combine(assetDirectory, "openswitch.ico");
        if (File.Exists(iconPath))
        {
            return new Icon(iconPath);
        }

        var imagePath = Path.Combine(assetDirectory, "openswitch.png");
        if (File.Exists(imagePath))
        {
            return LoadPngIcon(imagePath);
        }

        return CreateBrandIcon();
    }

    private static Icon CreateBrandIcon()
    {
        using var bitmap = new Bitmap(256, 256);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.SmoothingMode = SmoothingMode.AntiAlias;
        graphics.Clear(Color.Transparent);

        using var backgroundPath = new GraphicsPath();
        backgroundPath.AddArc(8, 8, 52, 52, 180, 90);
        backgroundPath.AddArc(196, 8, 52, 52, 270, 90);
        backgroundPath.AddArc(196, 196, 52, 52, 0, 90);
        backgroundPath.AddArc(8, 196, 52, 52, 90, 90);
        backgroundPath.CloseFigure();
        using var backgroundBrush = new SolidBrush(Color.FromArgb(8, 42, 78));
        using var borderPen = new Pen(Color.FromArgb(34, 93, 139), 5);
        graphics.FillPath(backgroundBrush, backgroundPath);
        graphics.DrawPath(borderPen, backgroundPath);

        using var arcPen = new Pen(Color.FromArgb(224, 233, 242), 22);
        graphics.DrawArc(arcPen, new Rectangle(38, 38, 180, 180), 0, 205);
        graphics.DrawArc(arcPen, new Rectangle(38, 38, 180, 180), 180, 155);
        using var arrowBrush = new SolidBrush(Color.FromArgb(248, 250, 252));
        graphics.FillPolygon(arrowBrush, new Point[] { new(52, 39), new(111, 39), new(77, 91) });
        graphics.FillPolygon(arrowBrush, new Point[] { new(204, 217), new(145, 217), new(179, 165) });

        using var font = new Font("Arial", 76, FontStyle.Bold, GraphicsUnit.Pixel);
        using var textBrush = new SolidBrush(Color.White);
        using var textFormat = new StringFormat
        {
            Alignment = StringAlignment.Center,
            LineAlignment = StringAlignment.Center
        };
        graphics.DrawString("T", font, textBrush, new RectangleF(73, 70, 110, 115), textFormat);

        return ToIcon(bitmap);
    }

    private static Icon LoadPngIcon(string path)
    {
        using var bitmap = new Bitmap(path);
        return ToIcon(bitmap);
    }

    private static Icon ToIcon(Bitmap bitmap)
    {
        var handle = bitmap.GetHicon();
        try
        {
            using var temporaryIcon = Icon.FromHandle(handle);
            return (Icon)temporaryIcon.Clone();
        }
        finally
        {
            DestroyIcon(handle);
        }
    }
    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyIcon(IntPtr handle);
}
