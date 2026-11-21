using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.Daemon;

/// <summary>
/// Why a settings route did not answer as asked.
/// </summary>
public enum SettingsFailureKind
{
    /// <summary>
    /// 409: <c>config.toml</c> changed since its revision was read.
    /// </summary>
    Conflict,

    /// <summary>
    /// 422: nothing was written. With problems, the values that were refused (or the file's own
    /// problems); without, the file is not TOML or not UTF-8.
    /// </summary>
    Invalid,

    /// <summary>
    /// 423: another program holds <c>config.toml</c>.
    /// </summary>
    Held,

    /// <summary>
    /// 503: allotd has no machine sample or no config file yet.
    /// </summary>
    Unavailable,

    /// <summary>
    /// No token file, or allotd refused the token after one fresh read.
    /// </summary>
    Token,

    /// <summary>
    /// Nothing answered in time.
    /// </summary>
    Unreachable,

    /// <summary>
    /// Anything else, such as a 500 or an answer that does not decode.
    /// </summary>
    Failed,
}

/// <summary>
/// A settings request that got no value.
/// </summary>
/// <param name="Kind">What went wrong.</param>
/// <param name="Message">allotd's sentence, or what went wrong on the way.</param>
/// <param name="Problems">The refused values, when values were the cause.</param>
public sealed record SettingsFailure(
    SettingsFailureKind Kind,
    string Message,
    IReadOnlyList<FieldError> Problems);

/// <summary>
/// The answer of a settings route: the decoded value, or why there is none. Exactly one of
/// <see cref="SettingsReply{T}.Value"/> and <see cref="SettingsReply{T}.Failure"/> is set.
/// </summary>
/// <typeparam name="T">The decoded body of a 200.</typeparam>
public sealed class SettingsReply<T>
    where T : class
{
    /// <summary>
    /// An answered request.
    /// </summary>
    public SettingsReply(T value)
    {
        Value = value;
    }

    /// <summary>
    /// A refused or unanswered request.
    /// </summary>
    public SettingsReply(SettingsFailure failure)
    {
        Failure = failure;
    }

    /// <summary>
    /// The decoded answer; <c>null</c> when the request failed.
    /// </summary>
    public T? Value { get; }

    /// <summary>
    /// Why there is no value; <c>null</c> when the request was answered.
    /// </summary>
    public SettingsFailure? Failure { get; }
}
