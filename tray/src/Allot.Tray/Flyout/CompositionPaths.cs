using System.Runtime.InteropServices;

using Allot.Tray.Interop;

using Microsoft.UI.Composition;

using Windows.Foundation;
using Windows.Graphics;

using WinRT;

namespace Allot.Tray.Flyout;

/// <summary>
/// One closed figure: a start point, then straight lines and cubic Béziers.
/// </summary>
public sealed class PathFigure(Point start)
{
    private readonly List<(bool Curve, Point P1, Point P2, Point P3)> _steps = [];

    public Point Start { get; } = start;

    public IReadOnlyList<(bool Curve, Point P1, Point P2, Point P3)> Steps => _steps;

    public PathFigure LineTo(double x, double y)
    {
        _steps.Add((false, new Point(x, y), default, default));
        return this;
    }

    public PathFigure CurveTo(Point control1, Point control2, Point end)
    {
        _steps.Add((true, control1, control2, end));
        return this;
    }
}

/// <summary>
/// Makes a <see cref="CompositionPath"/> without Win2D. Composition reads
/// a path's shape as a Direct2D geometry through <c>IGeometrySource2DInterop</c>, so this hands
/// it a small COM object built by hand that draws the figure into whichever D2D factory
/// composition asks with.
/// </summary>
internal static unsafe class CompositionPaths
{
    private const int Ok = 0;
    private const int NoInterface = unchecked((int)0x80004002);
    private const int FigureBeginFilled = 0;
    private const int FigureEndClosed = 1;

    // Vtable slots of the D2D interfaces used here (d2d1.h declaration order).
    private const int ReleaseSlot = 2;
    private const int CreatePathGeometrySlot = 10;
    private const int OpenSlot = 17;
    private const int BeginFigureSlot = 5;
    private const int EndFigureSlot = 8;
    private const int CloseSlot = 9;
    private const int AddLineSlot = 10;
    private const int AddBezierSlot = 11;

    private static readonly Guid IidUnknown = new("00000000-0000-0000-C000-000000000046");
    private static readonly Guid IidInspectable = new("AF86E2E0-B12D-4C6A-9C5A-D7AA65101E90");
    private static readonly Guid IidGeometrySource = typeof(IGeometrySource2D).GUID;
    private static readonly Guid IidGeometrySourceInterop =
        new("0657AF73-53FD-47CF-84FF-C8492D2A80A3");
    private static readonly Guid IidFactory = new("06152247-6F50-465A-9245-118BFD3B6007");

    // IUnknown, IInspectable and IGeometrySource2D (which adds nothing) share one vtable.
    private static readonly nint SourceVtable = SourceMethods();
    private static readonly nint InteropVtable = InteropMethods();

    public static CompositionPath Create(PathFigure figure)
    {
        var source = (Source*)NativeMemory.Alloc((nuint)sizeof(Source));
        source->Vtable = SourceVtable;
        source->InteropVtable = InteropVtable;
        source->Figure = GCHandle.ToIntPtr(GCHandle.Alloc(figure));
        source->References = 1;
        try
        {
            // The wrapper takes its own reference; ours is dropped once the path is made.
            var wrapper = MarshalInterface<IGeometrySource2D>.FromAbi((nint)source);
            return new CompositionPath(wrapper);
        }
        finally
        {
            _ = Release(source);
        }
    }

    private static nint SourceMethods()
    {
        var table = (nint*)NativeMemory.Alloc((nuint)(6 * sizeof(nint)));
        table[0] = (nint)(delegate* unmanaged<Source*, Guid*, void**, int>)&SourceQuery;
        table[1] = (nint)(delegate* unmanaged<Source*, uint>)&SourceAddRef;
        table[2] = (nint)(delegate* unmanaged<Source*, uint>)&SourceRelease;
        table[3] = (nint)(delegate* unmanaged<Source*, uint*, Guid**, int>)&GetIids;
        table[4] = (nint)(delegate* unmanaged<Source*, nint*, int>)&GetRuntimeClassName;
        table[5] = (nint)(delegate* unmanaged<Source*, int*, int>)&GetTrustLevel;
        return (nint)table;
    }

    private static nint InteropMethods()
    {
        var table = (nint*)NativeMemory.Alloc((nuint)(5 * sizeof(nint)));
        table[0] = (nint)(delegate* unmanaged<nint*, Guid*, void**, int>)&InteropQuery;
        table[1] = (nint)(delegate* unmanaged<nint*, uint>)&InteropAddRef;
        table[2] = (nint)(delegate* unmanaged<nint*, uint>)&InteropRelease;
        table[3] = (nint)(delegate* unmanaged<nint*, nint*, int>)&GetGeometry;
        table[4] = (nint)(delegate* unmanaged<nint*, nint, nint*, int>)&TryGetGeometryUsingFactory;
        return (nint)table;
    }

    // The interop interface pointer is the second field of the object.
    private static Source* Owner(nint* interop) => (Source*)((byte*)interop - sizeof(nint));

    private static int Query(Source* source, Guid* iid, void** result)
    {
        if (*iid == IidUnknown || *iid == IidInspectable || *iid == IidGeometrySource)
        {
            *result = source;
        }
        else if (*iid == IidGeometrySourceInterop)
        {
            *result = &source->InteropVtable;
        }
        else
        {
            *result = null;
            return NoInterface;
        }

        _ = Interlocked.Increment(ref source->References);
        return Ok;
    }

    private static uint Release(Source* source)
    {
        var left = Interlocked.Decrement(ref source->References);
        if (left == 0)
        {
            GCHandle.FromIntPtr(source->Figure).Free();
            NativeMemory.Free(source);
        }

        return (uint)left;
    }

    [UnmanagedCallersOnly]
    private static int SourceQuery(Source* source, Guid* iid, void** result) =>
        Query(source, iid, result);

    [UnmanagedCallersOnly]
    private static uint SourceAddRef(Source* source) =>
        (uint)Interlocked.Increment(ref source->References);

    [UnmanagedCallersOnly]
    private static uint SourceRelease(Source* source) => Release(source);

    [UnmanagedCallersOnly]
    private static int GetIids(Source* source, uint* count, Guid** iids)
    {
        *count = 0;
        *iids = null;
        return Ok;
    }

    [UnmanagedCallersOnly]
    private static int GetRuntimeClassName(Source* source, nint* name)
    {
        *name = 0;
        return Ok;
    }

    [UnmanagedCallersOnly]
    private static int GetTrustLevel(Source* source, int* level)
    {
        *level = 0;
        return Ok;
    }

    [UnmanagedCallersOnly]
    private static int InteropQuery(nint* interop, Guid* iid, void** result) =>
        Query(Owner(interop), iid, result);

    [UnmanagedCallersOnly]
    private static uint InteropAddRef(nint* interop) =>
        (uint)Interlocked.Increment(ref Owner(interop)->References);

    [UnmanagedCallersOnly]
    private static uint InteropRelease(nint* interop) => Release(Owner(interop));

    [UnmanagedCallersOnly]
    private static int GetGeometry(nint* interop, nint* geometry)
    {
        *geometry = 0;
        var hr = NativeMethods.D2D1CreateFactory(
            NativeMethods.D2D1FactoryTypeMultiThreaded, IidFactory, 0, out var factory);
        if (hr < 0)
        {
            return hr;
        }

        hr = Build(FigureOf(interop), factory, geometry);
        _ = ReleaseCom(factory);
        return hr;
    }

    [UnmanagedCallersOnly]
    private static int TryGetGeometryUsingFactory(nint* interop, nint factory, nint* geometry)
    {
        *geometry = 0;
        return Build(FigureOf(interop), factory, geometry);
    }

    private static PathFigure FigureOf(nint* interop) =>
        (PathFigure)GCHandle.FromIntPtr(Owner(interop)->Figure).Target!;

    // ID2D1Factory::CreatePathGeometry, ID2D1PathGeometry::Open, then the figure into the sink.
    private static int Build(PathFigure figure, nint factory, nint* geometry)
    {
        nint path;
        var hr = ((delegate* unmanaged<nint, nint*, int>)Slot(factory, CreatePathGeometrySlot))(
            factory, &path);
        if (hr < 0)
        {
            return hr;
        }

        nint sink;
        hr = ((delegate* unmanaged<nint, nint*, int>)Slot(path, OpenSlot))(path, &sink);
        if (hr < 0)
        {
            _ = ReleaseCom(path);
            return hr;
        }

        ((delegate* unmanaged<nint, Point2F, int, void>)Slot(sink, BeginFigureSlot))(
            sink, Point2F.From(figure.Start), FigureBeginFilled);
        foreach (var (curve, p1, p2, p3) in figure.Steps)
        {
            if (curve)
            {
                var bezier = new Bezier(Point2F.From(p1), Point2F.From(p2), Point2F.From(p3));
                ((delegate* unmanaged<nint, Bezier*, void>)Slot(sink, AddBezierSlot))(
                    sink, &bezier);
            }
            else
            {
                ((delegate* unmanaged<nint, Point2F, void>)Slot(sink, AddLineSlot))(
                    sink, Point2F.From(p1));
            }
        }

        ((delegate* unmanaged<nint, int, void>)Slot(sink, EndFigureSlot))(sink, FigureEndClosed);
        hr = ((delegate* unmanaged<nint, int>)Slot(sink, CloseSlot))(sink);
        _ = ReleaseCom(sink);
        if (hr < 0)
        {
            _ = ReleaseCom(path);
            return hr;
        }

        // The path geometry is an ID2D1Geometry, and the caller owns this reference.
        *geometry = path;
        return Ok;
    }

    private static nint Slot(nint instance, int slot) => (*(nint**)instance)[slot];

    private static uint ReleaseCom(nint instance) =>
        ((delegate* unmanaged<nint, uint>)Slot(instance, ReleaseSlot))(instance);

    [StructLayout(LayoutKind.Sequential)]
    private struct Source
    {
        public nint Vtable;
        public nint InteropVtable;
        public nint Figure;
        public int References;
    }

    [StructLayout(LayoutKind.Sequential)]
    private readonly struct Point2F(float x, float y)
    {
        public readonly float X = x;
        public readonly float Y = y;

        public static Point2F From(Point point) => new((float)point.X, (float)point.Y);
    }

    [StructLayout(LayoutKind.Sequential)]
    private readonly struct Bezier(Point2F point1, Point2F point2, Point2F point3)
    {
        public readonly Point2F Point1 = point1;
        public readonly Point2F Point2 = point2;
        public readonly Point2F Point3 = point3;
    }
}
