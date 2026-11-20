using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The lease classes as <c>config.toml</c> names them, such as <c>cpu_heavy</c>.
/// </summary>
public static class ClassNames
{
    public static string Of(JobClass jobClass) => jobClass switch
    {
        JobClass.Light => "light",
        JobClass.CpuHeavy => "cpu_heavy",
        JobClass.GpuHeavy => "gpu_heavy",
        JobClass.Mixed => "mixed",
        _ => throw new ArgumentOutOfRangeException(nameof(jobClass), jobClass, null),
    };

    /// <returns><c>null</c> for a name that is not a class.</returns>
    public static JobClass? Parse(string? name) => name switch
    {
        "light" => JobClass.Light,
        "cpu_heavy" => JobClass.CpuHeavy,
        "gpu_heavy" => JobClass.GpuHeavy,
        "mixed" => JobClass.Mixed,
        _ => null,
    };
}
