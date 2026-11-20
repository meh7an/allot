using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A size in megabytes, shown in MB or GB; the file always gets MB.
/// </summary>
public sealed class MemoryModel(SettingSpec spec, IValueHost host, double? value)
    : NumberModel(spec, host, value, ValueUnit.Memory);
