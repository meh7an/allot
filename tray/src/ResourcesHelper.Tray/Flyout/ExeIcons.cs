using System.Runtime.InteropServices.WindowsRuntime;

using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

using ResourcesHelper.Tray.Interop;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// Executable icons for the process tiles. Each one is extracted on a pool thread at the tile's
/// physical size and cached by path and size for the app's life, so an executable is read once
/// however often its job polls; a failure is cached too, and the tile keeps its initials. Called
/// on the UI thread only.
/// </summary>
internal static class ExeIcons
{
    private static readonly Dictionary<string, Task<ImageSource?>> Cache =
        new(StringComparer.OrdinalIgnoreCase);

    /// <param name="path">The executable's full path.</param>
    /// <param name="size">The icon's size in physical pixels.</param>
    /// <returns>The icon, or <c>null</c> when the file has none or cannot be read.</returns>
    public static Task<ImageSource?> GetAsync(string path, int size)
    {
        var key = $"{size}|{path}";
        if (!Cache.TryGetValue(key, out var icon))
        {
            icon = LoadAsync(path, size);
            Cache[key] = icon;
        }

        return icon;
    }

    private static async Task<ImageSource?> LoadAsync(string path, int size)
    {
        IconPixels? pixels;
        try
        {
            // Resumes on the UI thread, which owns the bitmap.
            pixels = await Task.Run(() => Extract(path, size));
        }
        catch (Exception exception) when (exception is not OutOfMemoryException)
        {
            AppLog.Write($"icon of {path}: {exception.Message}");
            return null;
        }

        if (pixels is null)
        {
            return null;
        }

        var bitmap = new WriteableBitmap(pixels.Width, pixels.Height);
        using (var stream = bitmap.PixelBuffer.AsStream())
        {
            stream.Write(pixels.Bgra.Span);
        }

        bitmap.Invalidate();
        return bitmap;
    }

    private static unsafe IconPixels? Extract(string path, int size)
    {
        if (NativeMethods.SHDefExtractIcon(path, 0, 0, out var icon, 0, (uint)size) != 0
            || icon == 0)
        {
            return null;
        }

        try
        {
            if (!NativeMethods.GetIconInfo(icon, out var info))
            {
                return null;
            }

            try
            {
                // Monochrome icons have no colour bitmap; the tile keeps its initials.
                NativeMethods.NativeBitmap bitmap;
                var bitmapSize = sizeof(NativeMethods.NativeBitmap);
                if (info.Color == 0
                    || NativeMethods.GetObject(info.Color, bitmapSize, &bitmap) == 0)
                {
                    return null;
                }

                var (width, height) = (bitmap.Width, bitmap.Height);
                var color = ReadBits(info.Color, width, height);
                if (color is null)
                {
                    return null;
                }

                // Icons without an alpha channel carry their transparency in the AND mask.
                if (!HasAlpha(color))
                {
                    var mask = ReadBits(info.Mask, width, height);
                    if (mask is null)
                    {
                        return null;
                    }

                    ApplyMask(color, mask);
                }

                Premultiply(color);
                return new IconPixels(width, height, color);
            }
            finally
            {
                DeleteBitmap(info.Color);
                DeleteBitmap(info.Mask);
            }
        }
        finally
        {
            _ = NativeMethods.DestroyIcon(icon);
        }
    }

    // Top-down 32-bit BGRA rows of a bitmap.
    private static unsafe byte[]? ReadBits(nint bitmap, int width, int height)
    {
        var dc = NativeMethods.CreateCompatibleDC(0);
        if (dc == 0)
        {
            return null;
        }

        try
        {
            // Room for the colour table GetDIBits may write after the header for a mask.
            var buffer = stackalloc byte[sizeof(NativeMethods.BitmapInfoHeader) + (256 * 4)];
            var header = (NativeMethods.BitmapInfoHeader*)buffer;
            *header = new NativeMethods.BitmapInfoHeader
            {
                Size = (uint)sizeof(NativeMethods.BitmapInfoHeader),
                Width = width,
                Height = -height,
                Planes = 1,
                BitCount = 32,
                Compression = NativeMethods.BiRgb,
            };
            var bits = new byte[width * height * 4];
            fixed (byte* target = bits)
            {
                var lines = NativeMethods.GetDIBits(
                    dc, bitmap, 0, (uint)height, target, header, NativeMethods.DibRgbColors);
                return lines == height ? bits : null;
            }
        }
        finally
        {
            _ = NativeMethods.DeleteDC(dc);
        }
    }

    private static bool HasAlpha(byte[] bgra)
    {
        for (var i = 3; i < bgra.Length; i += 4)
        {
            if (bgra[i] != 0)
            {
                return true;
            }
        }

        return false;
    }

    // A white mask pixel is transparent, a black one opaque.
    private static void ApplyMask(byte[] bgra, byte[] mask)
    {
        for (var i = 0; i < bgra.Length; i += 4)
        {
            bgra[i + 3] = mask[i] == 0 ? (byte)255 : (byte)0;
        }
    }

    // WriteableBitmap takes premultiplied alpha; icons store it straight.
    private static void Premultiply(byte[] bgra)
    {
        for (var i = 0; i < bgra.Length; i += 4)
        {
            var alpha = bgra[i + 3];
            bgra[i] = (byte)(bgra[i] * alpha / 255);
            bgra[i + 1] = (byte)(bgra[i + 1] * alpha / 255);
            bgra[i + 2] = (byte)(bgra[i + 2] * alpha / 255);
        }
    }

    private static void DeleteBitmap(nint bitmap)
    {
        if (bitmap != 0)
        {
            _ = NativeMethods.DeleteObject(bitmap);
        }
    }

    private sealed record IconPixels(int Width, int Height, ReadOnlyMemory<byte> Bgra);
}
