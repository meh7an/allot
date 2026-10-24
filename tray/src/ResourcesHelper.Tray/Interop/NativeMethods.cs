using System.Runtime.InteropServices;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]

namespace ResourcesHelper.Tray.Interop;

/// <summary>
/// The few Win32 calls the Windows App SDK has no wrapper for.
/// </summary>
internal static partial class NativeMethods
{
    public const int DwmwaExtendedFrameBounds = 9;
    public const int DwmwaWindowCornerPreference = 33;
    public const int DwmwaBorderColor = 34;
    public const int DwmwcpRound = 2;
    public const int DwmwaColorNone = unchecked((int)0xFFFFFFFE);

    public const int MdtEffectiveDpi = 0;

    public const uint WmNull = 0x0000;
    public const uint WmSettingChange = 0x001A;
    public const uint WmDisplayChange = 0x007E;
    public const uint WmDpiChanged = 0x02E0;
    public const uint WmNcRButtonDown = 0x00A4;
    public const uint WmNcRButtonUp = 0x00A5;
    public const uint WmMoving = 0x0216;
    public const uint WmEnterSizeMove = 0x0231;
    public const uint WmExitSizeMove = 0x0232;
    public const nint HtCaption = 2;
    public const uint WmApp = 0x8000;

    public const uint WsPopup = 0x80000000;
    public const uint WsExToolWindow = 0x00000080;

    public const uint NimAdd = 0;
    public const uint NimModify = 1;
    public const uint NimDelete = 2;
    public const uint NimSetVersion = 4;
    public const uint NifMessage = 0x01;
    public const uint NifIcon = 0x02;
    public const uint NifTip = 0x04;
    public const uint NifShowTip = 0x80;
    public const uint NotifyIconVersion4 = 4;

    public const uint ImageIcon = 1;
    public const uint LrLoadFromFile = 0x10;
    public const int SmCxSmIcon = 49;
    public const int SmMenuDropAlignment = 40;
    public const uint MonitorDefaultToPrimary = 1;
    public const uint MonitorDefaultToNearest = 2;

    public const uint MbYesNo = 0x0004;
    public const uint MbIconWarning = 0x0030;
    public const uint MbDefButton2 = 0x0100;
    public const uint MbSetForeground = 0x00010000;
    public const int IdYes = 6;

    public const uint DibRgbColors = 0;
    public const uint BiRgb = 0;

    public const uint MfString = 0x0000;
    public const uint MfGrayed = 0x0001;
    public const uint MfChecked = 0x0008;
    public const uint MfPopup = 0x0010;
    public const uint MfSeparator = 0x0800;
    public const uint TpmLeftAlign = 0x0000;
    public const uint TpmRightButton = 0x0002;
    public const uint TpmRightAlign = 0x0008;
    public const uint TpmBottomAlign = 0x0020;
    public const uint TpmReturnCmd = 0x0100;

    [LibraryImport("dwmapi.dll")]
    public static partial int DwmSetWindowAttribute(
        nint hwnd,
        int attribute,
        in int value,
        int size);

    [LibraryImport("dwmapi.dll")]
    public static partial int DwmGetWindowAttribute(
        nint hwnd,
        int attribute,
        out NativeRect value,
        int size);

    [LibraryImport("shcore.dll")]
    public static partial int GetDpiForMonitor(
        nint monitor,
        int dpiType,
        out uint dpiX,
        out uint dpiY);

    [LibraryImport("user32.dll")]
    public static partial uint GetDpiForWindow(nint hwnd);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool GetWindowRect(nint hwnd, out NativeRect rect);

    [LibraryImport("user32.dll")]
    public static partial nint GetForegroundWindow();

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetForegroundWindow(nint hwnd);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool AllowSetForegroundWindow(uint processId);

    [LibraryImport("user32.dll")]
    public static partial uint GetWindowThreadProcessId(nint hwnd, out uint processId);

    [LibraryImport("user32.dll")]
    public static partial nint MonitorFromPoint(NativePoint point, uint flags);

    [LibraryImport("user32.dll")]
    public static partial nint MonitorFromRect(in NativeRect rect, uint flags);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool GetCursorPos(out NativePoint point);

    [LibraryImport("user32.dll", EntryPoint = "GetMonitorInfoW")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool GetMonitorInfo(nint monitor, ref MonitorInfo info);

    [LibraryImport(
        "user32.dll",
        EntryPoint = "MessageBoxW",
        StringMarshalling = StringMarshalling.Utf16)]
    public static partial int MessageBox(nint owner, string text, string caption, uint type);

    /// <summary>
    /// Needs Common Controls 6, which app.manifest asks for.
    /// </summary>
    [LibraryImport("comctl32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static unsafe partial bool SetWindowSubclass(
        nint hwnd,
        delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> subclassProc,
        nuint id,
        nuint referenceData);

    [LibraryImport("comctl32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static unsafe partial bool RemoveWindowSubclass(
        nint hwnd,
        delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> subclassProc,
        nuint id);

    [LibraryImport("comctl32.dll")]
    public static partial nint DefSubclassProc(nint hwnd, uint message, nint wParam, nint lParam);

    [LibraryImport("user32.dll")]
    public static partial int GetSystemMetrics(int index);

    [LibraryImport("user32.dll")]
    public static partial int GetSystemMetricsForDpi(int index, uint dpi);

    [LibraryImport("shell32.dll", EntryPoint = "Shell_NotifyIconW")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static unsafe partial bool ShellNotifyIcon(uint message, NotifyIconData* data);

    [LibraryImport("user32.dll", EntryPoint = "RegisterClassExW", SetLastError = true)]
    public static unsafe partial ushort RegisterClassEx(WindowClass* windowClass);

    [LibraryImport(
        "user32.dll",
        EntryPoint = "CreateWindowExW",
        StringMarshalling = StringMarshalling.Utf16,
        SetLastError = true)]
    public static partial nint CreateWindowEx(
        uint exStyle,
        string className,
        string windowName,
        uint style,
        int x,
        int y,
        int width,
        int height,
        nint parent,
        nint menu,
        nint instance,
        nint param);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool DestroyWindow(nint hwnd);

    [LibraryImport("user32.dll", EntryPoint = "DefWindowProcW")]
    public static partial nint DefWindowProc(nint hwnd, uint message, nint wParam, nint lParam);

    [LibraryImport("user32.dll", EntryPoint = "PostMessageW")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool PostMessage(nint hwnd, uint message, nint wParam, nint lParam);

    [LibraryImport(
        "user32.dll",
        EntryPoint = "RegisterWindowMessageW",
        StringMarshalling = StringMarshalling.Utf16)]
    public static partial uint RegisterWindowMessage(string name);

    [LibraryImport(
        "user32.dll",
        EntryPoint = "LoadImageW",
        StringMarshalling = StringMarshalling.Utf16,
        SetLastError = true)]
    public static partial nint LoadImage(
        nint instance,
        string name,
        uint type,
        int width,
        int height,
        uint flags);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool DestroyIcon(nint icon);

    [LibraryImport("user32.dll")]
    public static partial nint CreatePopupMenu();

    [LibraryImport(
        "user32.dll",
        EntryPoint = "AppendMenuW",
        StringMarshalling = StringMarshalling.Utf16)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool AppendMenu(nint menu, uint flags, nuint id, string? text);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetMenuDefaultItem(nint menu, uint item, uint byPosition);

    [LibraryImport("user32.dll")]
    public static partial int TrackPopupMenuEx(
        nint menu,
        uint flags,
        int x,
        int y,
        nint hwnd,
        nint parameters);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool DestroyMenu(nint menu);

    [LibraryImport(
        "kernel32.dll",
        EntryPoint = "GetModuleHandleW",
        StringMarshalling = StringMarshalling.Utf16)]
    public static partial nint GetModuleHandle(string? moduleName);

    /// <summary>
    /// GetProcAddress by ordinal: the name argument carries the ordinal in its low word.
    /// </summary>
    [LibraryImport("kernel32.dll")]
    public static partial nint GetProcAddress(nint module, nint ordinal);

    /// <summary>
    /// Extracts icon <paramref name="index"/> of a file at <paramref name="size"/> pixels (the
    /// low word; the high word is the small icon's size, unused here). S_FALSE when the file has
    /// no icon.
    /// </summary>
    [LibraryImport(
        "shell32.dll",
        EntryPoint = "SHDefExtractIconW",
        StringMarshalling = StringMarshalling.Utf16)]
    public static partial int SHDefExtractIcon(
        string iconFile,
        int index,
        uint flags,
        out nint large,
        nint small,
        uint size);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool GetIconInfo(nint icon, out IconInfo info);

    [LibraryImport("gdi32.dll", EntryPoint = "GetObjectW")]
    public static unsafe partial int GetObject(nint gdiObject, int size, void* buffer);

    [LibraryImport("gdi32.dll")]
    public static unsafe partial int GetDIBits(
        nint dc,
        nint bitmap,
        uint start,
        uint lines,
        void* bits,
        BitmapInfoHeader* info,
        uint usage);

    [LibraryImport("gdi32.dll")]
    public static partial nint CreateCompatibleDC(nint dc);

    [LibraryImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool DeleteDC(nint dc);

    [LibraryImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool DeleteObject(nint gdiObject);

    [StructLayout(LayoutKind.Sequential)]
    public struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    /// <summary>
    /// MONITORINFO.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct MonitorInfo
    {
        public uint Size;
        public NativeRect Monitor;
        public NativeRect Work;
        public uint Flags;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NativePoint
    {
        public int X;
        public int Y;
    }

    /// <summary>
    /// WNDCLASSEXW.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public unsafe struct WindowClass
    {
        public uint Size;
        public uint Style;
        public delegate* unmanaged<nint, uint, nint, nint, nint> WindowProc;
        public int ClassExtra;
        public int WindowExtra;
        public nint Instance;
        public nint Icon;
        public nint Cursor;
        public nint Background;
        public char* MenuName;
        public char* ClassName;
        public nint SmallIcon;
    }

    /// <summary>
    /// NOTIFYICONDATAW, passed by pointer so the UTF-16 buffers need no marshalling; 976 bytes on
    /// x64.
    /// </summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public unsafe struct NotifyIconData
    {
        public uint Size;
        public nint Window;
        public uint Id;
        public uint Flags;
        public uint CallbackMessage;
        public nint Icon;
        public fixed char Tip[128];
        public uint State;
        public uint StateMask;
        public fixed char Info[256];
        public uint TimeoutOrVersion;
        public fixed char InfoTitle[64];
        public uint InfoFlags;
        public Guid Item;
        public nint BalloonIcon;
    }

    /// <summary>
    /// ICONINFO.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct IconInfo
    {
        public int IsIcon;
        public uint HotspotX;
        public uint HotspotY;
        public nint Mask;
        public nint Color;
    }

    /// <summary>
    /// BITMAP, as GetObject fills it for a bitmap handle.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct NativeBitmap
    {
        public int Type;
        public int Width;
        public int Height;
        public int WidthBytes;
        public ushort Planes;
        public ushort BitsPixel;
        public nint Bits;
    }

    /// <summary>
    /// BITMAPINFOHEADER; a negative height asks for top-down rows.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct BitmapInfoHeader
    {
        public uint Size;
        public int Width;
        public int Height;
        public ushort Planes;
        public ushort BitCount;
        public uint Compression;
        public uint SizeImage;
        public int XPelsPerMeter;
        public int YPelsPerMeter;
        public uint ClrUsed;
        public uint ClrImportant;
    }
}
