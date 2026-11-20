import AllotContract
import Foundation

/// Builds the contract values the settings view models write. The generated types have no
/// initializers outside their module, so each is encoded as the JSON it is defined by and
/// decoded again.
public enum Contract {
    // MARK: Changes

    public static func setSwitch(_ key: String, _ value: Bool) -> Change {
        build(Change.self, SetSwitchWire(key: key, value: value))
    }

    /// - Precondition: `value` is finite; the models check it before they build a change.
    public static func setNumber(_ key: String, _ value: Double) -> Change {
        build(Change.self, SetNumberWire(key: key, value: value))
    }

    public static func setList(_ key: String, _ value: [String]) -> Change {
        build(Change.self, SetListWire(key: key, value: value))
    }

    public static func setRules(_ rules: [HeavyRule]) -> Change {
        build(Change.self, SetRulesWire(rules: rules))
    }

    public static func setRow(table: String, row: SizeRow) -> Change {
        build(
            Change.self,
            SetRowWire(table: table, name: row.name, profile: row.profile, minCores: row.minCores))
    }

    public static func resetRow(table: String, name: String) -> Change {
        build(Change.self, ResetRowWire(table: table, name: name))
    }

    public static func reset(_ key: String) -> Change {
        build(Change.self, ResetWire(key: key))
    }

    // MARK: Values

    /// A rule of the heavy-command list.
    public static func rule(
        program: String,
        args: [String],
        class ruleClass: JobClass
    ) -> HeavyRule {
        build(
            HeavyRule.self,
            RuleWire(program: program, args: args, ruleClass: ruleClass.rawValue))
    }

    /// A job profile from the numbers of a row's fields; negative and fractional values are
    /// brought to the whole numbers the file holds.
    public static func profile(
        cores: Double,
        ramMb: Double,
        gpuPct: Double,
        vramMb: Double,
        estSecs: Double
    ) -> JobProfile {
        build(
            JobProfile.self,
            ProfileWire(
                cores: Float(cores),
                ramMb: whole(ramMb),
                gpuPct: Float(gpuPct),
                vramMb: whole(vramMb),
                estSecs: whole(estSecs)))
    }

    /// A row of a size table.
    public static func sizeRow(name: String, profile: JobProfile, minCores: Double?) -> SizeRow {
        build(
            SizeRow.self,
            SizeRowWire(name: name, profile: profile, minCores: minCores.map { Float($0) }))
    }

    private static func whole(_ value: Double) -> UInt64 {
        value.isFinite && value > 0 ? UInt64(min(value.rounded(), 1e18)) : 0
    }

    private static func build<Value: Decodable, Wire: Encodable>(
        _ type: Value.Type,
        _ wire: Wire
    ) -> Value {
        do {
            return try JSONDecoder().decode(type, from: JSONEncoder().encode(wire))
        } catch {
            // The wire values above hold only numbers the callers have checked.
            preconditionFailure("\(Value.self) could not be built: \(error)")
        }
    }
}

// The JSON of each contract value, in the shape `contract/` defines.

private struct SetSwitchWire: Encodable {
    let kind = "set_switch"
    let key: String
    let value: Bool
}

private struct SetNumberWire: Encodable {
    let kind = "set_number"
    let key: String
    let value: Double
}

private struct SetListWire: Encodable {
    let kind = "set_list"
    let key: String
    let value: [String]
}

private struct SetRulesWire: Encodable {
    let kind = "set_rules"
    let rules: [HeavyRule]
}

private struct SetRowWire: Encodable {
    let kind = "set_row"
    let table: String
    let name: String
    let profile: JobProfile
    let minCores: Float?

    private enum CodingKeys: String, CodingKey {
        case kind
        case table
        case name
        case profile
        case minCores = "min_cores"
    }
}

private struct ResetRowWire: Encodable {
    let kind = "reset_row"
    let table: String
    let name: String
}

private struct ResetWire: Encodable {
    let kind = "reset"
    let key: String
}

private struct RuleWire: Encodable {
    let program: String
    let args: [String]
    let ruleClass: String

    private enum CodingKeys: String, CodingKey {
        case program
        case args
        case ruleClass = "class"
    }
}

private struct ProfileWire: Encodable {
    let cores: Float
    let ramMb: UInt64
    let gpuPct: Float
    let vramMb: UInt64
    let estSecs: UInt64

    private enum CodingKeys: String, CodingKey {
        case cores
        case ramMb = "ram_mb"
        case gpuPct = "gpu_pct"
        case vramMb = "vram_mb"
        case estSecs = "est_secs"
    }
}

private struct SizeRowWire: Encodable {
    let name: String
    let profile: JobProfile
    let minCores: Float?

    private enum CodingKeys: String, CodingKey {
        case name
        case profile
        case minCores = "min_cores"
    }
}
