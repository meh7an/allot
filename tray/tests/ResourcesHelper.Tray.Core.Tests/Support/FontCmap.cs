using System.Buffers.Binary;

namespace ResourcesHelper.Tray.Core.Tests.Support;

/// <summary>
/// Reads which codepoints a TrueType font maps to a glyph, from its Windows Unicode cmap
/// subtable (format 4 or 12), so a test can prove the subset icon fonts kept every glyph.
/// </summary>
internal static class FontCmap
{
    public static bool Maps(string path, int codepoint)
    {
        var font = File.ReadAllBytes(path);
        var cmap = TableOffset(font, "cmap");
        var count = U16(font, cmap + 2);
        for (var i = 0; i < count; i++)
        {
            var record = cmap + 4 + (i * 8);
            var (platform, encoding) = (U16(font, record), U16(font, record + 2));
            if (platform != 3 || encoding is not (1 or 10))
            {
                continue;
            }

            var table = cmap + (int)U32(font, record + 4);
            return U16(font, table) switch
            {
                4 => Format4(font, table, codepoint),
                12 => Format12(font, table, codepoint),
                _ => false,
            };
        }

        return false;
    }

    private static int TableOffset(byte[] font, string tag)
    {
        var count = U16(font, 4);
        for (var i = 0; i < count; i++)
        {
            var record = 12 + (i * 16);
            if (System.Text.Encoding.ASCII.GetString(font, record, 4) == tag)
            {
                return (int)U32(font, record + 8);
            }
        }

        throw new InvalidDataException($"no {tag} table");
    }

    private static bool Format4(byte[] font, int table, int codepoint)
    {
        var segments = U16(font, table + 6) / 2;
        var ends = table + 14;
        var starts = ends + (segments * 2) + 2;
        var deltas = starts + (segments * 2);
        var rangeOffsets = deltas + (segments * 2);
        for (var i = 0; i < segments; i++)
        {
            var (start, end) = (U16(font, starts + (i * 2)), U16(font, ends + (i * 2)));
            if (codepoint < start || codepoint > end)
            {
                continue;
            }

            var delta = (short)U16(font, deltas + (i * 2));
            var rangeAt = rangeOffsets + (i * 2);
            var rangeOffset = U16(font, rangeAt);
            if (rangeOffset == 0)
            {
                return ((codepoint + delta) & 0xFFFF) != 0;
            }

            var glyph = U16(font, rangeAt + rangeOffset + ((codepoint - start) * 2));
            return glyph != 0;
        }

        return false;
    }

    private static bool Format12(byte[] font, int table, int codepoint)
    {
        var groups = U32(font, table + 12);
        for (var i = 0; i < groups; i++)
        {
            var group = table + 16 + (i * 12);
            if (codepoint >= U32(font, group) && codepoint <= U32(font, group + 4))
            {
                return true;
            }
        }

        return false;
    }

    private static int U16(byte[] font, int at) =>
        BinaryPrimitives.ReadUInt16BigEndian(font.AsSpan(at, 2));

    private static uint U32(byte[] font, int at) =>
        BinaryPrimitives.ReadUInt32BigEndian(font.AsSpan(at, 4));
}
