using System.Text.Json;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

// Release metadata rules shared by `version check`, `version bump`, and the release-tag commands.
internal static partial class VersionCommands
{
    // Repository-relative paths, `/`-separated as `Validate` reads them.
    private const string ConfiguratorPackageConfig =
        RepositoryPaths.PackagingDirectory + "/" + RepositoryNames.ConfiguratorPackageConfigFile;
    private const string PackageBuildPath = RepositoryPaths.PackagingDirectory + "/" + RepositoryNames.PackageBuildFile;
    private const string SourceInfoPath = RepositoryPaths.PackagingDirectory + "/" + RepositoryNames.SourceInfoFile;
    private const string ShellAurUpdater = "tools/update-aur-from-manifest.sh";
    private const string ReleaseWorkflow = ".github/workflows/build-packages.yml";
    private const string ReleasePackageJob = "package";
    private const string ValueGroup = "value";
    private const string OtherGroup = "other";
    private const string ItemsGroup = "items";
    private const string TomlStringArrayToken = @"""(?<value>[^""\\\n]*)""|'(?<value>[^'\n]*)'|#[^\n]*|[,\s]+|(?<other>.)";

    // A floor raise must add a reviewed runner contract instead of inheriting the
    // previous Ubuntu base image by accident.
    private static readonly IReadOnlyDictionary<string, string> ReleaseRunnerLibadwaitaFloors =
        new Dictionary<string, string>( StringComparer.Ordinal ) { [PackagingPlatform.UbuntuRunner] = "1.4" };

    private static readonly (string Label, string Path, string Pattern)[] LibadwaitaFloorSurfaces =
    [
        ("configurator deb libadwaita floor", ConfiguratorPackageConfig, @"^\s*-\s*libadwaita-1-0 \(>= ([0-9]+\.[0-9]+)\)\s*$"),
        ("configurator rpm libadwaita floor", ConfiguratorPackageConfig, @"^\s*-\s*libadwaita >= ([0-9]+\.[0-9]+)\s*$"),
        ("packaging/PKGBUILD libadwaita floor", PackageBuildPath, @"^\s*'libadwaita>=([0-9]+\.[0-9]+)'\s*$"),
        ("packaging/.SRCINFO libadwaita floor", SourceInfoPath, @"^\s*depends = libadwaita>=([0-9]+\.[0-9]+)\s*$"),
        ("AUR updater generated libadwaita floor", ShellAurUpdater,
            @"^\s*ensure_runtime_dependency 'libadwaita>=([0-9]+\.[0-9]+)' gcc-libs\s*$"),
        ("AUR updater PKGBUILD validation floor", ShellAurUpdater,
            @"^\s*&& grep -Eq ""[^""\n]*libadwaita>=([0-9]+\.[0-9]+)[^""\n]*"" PKGBUILD"),
        ("AUR updater .SRCINFO validation floor", ShellAurUpdater,
            @"^\s*&& grep -Fxq .*depends = libadwaita>=([0-9]+\.[0-9]+).*\.SRCINFO"),
    ];

    // Install examples that pin a concrete tag are stale one release later.
    private static readonly (string Pattern, string Label)[] ReadmePinPatterns =
    [
        (@"wayscriber\?ref=v?\d+\.\d+\.\d+(?:\.\d+)?", "pinned flake ref"),
        (@"/releases/(?:tag|download)/v?\d+\.\d+\.\d+(?:\.\d+)?", "pinned release URL"),
    ];

    internal static void EnsureConsistent( string root, string? releaseText )
    {
        var errors = Validate( root, releaseText );
        if ( errors.Count > 0 )
        {
            throw new ToolException( "Version consistency check failed:\n" + string.Join( '\n', errors.Select( error => $"- {error}" ) ) );
        }
    }

    // Commands always use SupportedLibadwaitaFloor; another floor models a coordinated
    // floor raise, which must still be rejected until the release runner contract changes.
    internal static List<string> Validate( string root, string? releaseText = null,
        string supportedLibadwaitaFloor = SupportedLibadwaitaFloor )
    {
        string PathFor( string relativePath ) => Path.Combine( [root, .. relativePath.Split( '/' )] );
        string Read( string relativePath ) => Files.Read( PathFor( relativePath ) );

        var errors = new List<string>( );
        var cargo = ReadCargoVersion( PathFor( RepositoryPaths.CargoManifest ) );
        var configurator = ReadCargoVersion( PathFor( RepositoryPaths.ConfiguratorCargoManifest ) );

        RequireEqual( errors, RepositoryPaths.ConfiguratorCargoManifest, configurator, cargo );
        RequireEqual( errors, $"{RepositoryNames.GlobalJsonFile} SDK", ReadToolSdk( Read, errors ), ToolSdkVersion );
        ValidateLibadwaitaFloors( Read, supportedLibadwaitaFloor, errors );
        ValidateReleaseRunner( Read( ReleaseWorkflow ), supportedLibadwaitaFloor, errors );
        ValidatePackageVersions( Read, cargo, releaseText, errors );
        ValidateFlake( Read( RepositoryNames.FlakeFile ), errors );
        ValidateReadme( Read( RepositoryNames.ReadmeFile ), errors );

        return errors;
    }

    private static string? ReadToolSdk( Func<string, string> read, List<string> errors )
    {
        JsonElement sdk;
        JsonElement version;
        try
        {
            using var document = JsonDocument.Parse( read( RepositoryNames.GlobalJsonFile ) );
            sdk = document.RootElement.GetProperty( "sdk" ).Clone( );
            version = sdk.GetProperty( "version" );
        }
        catch ( Exception error ) when ( error is IOException or JsonException or KeyNotFoundException or InvalidOperationException )
        {
            errors.Add( $"{RepositoryNames.GlobalJsonFile} SDK metadata is invalid: {error.Message}" );
            return null;
        }

        if ( version.ValueKind != JsonValueKind.String || string.IsNullOrWhiteSpace( version.GetString( ) ) )
        {
            errors.Add( $"{RepositoryNames.GlobalJsonFile} SDK metadata is invalid: sdk.version must be a non-empty string" );
            return null;
        }

        if ( !sdk.TryGetProperty( "rollForward", out var rollForward ) || rollForward.ValueKind != JsonValueKind.String ||
             rollForward.GetString( ) != ToolSdkRollForward )
        {
            errors.Add( $"{RepositoryNames.GlobalJsonFile} SDK rollForward must be {ToolSdkRollForward}" );
        }
        if ( !sdk.TryGetProperty( "allowPrerelease", out var allowPrerelease ) || allowPrerelease.ValueKind != JsonValueKind.True )
        {
            errors.Add( $"{RepositoryNames.GlobalJsonFile} SDK allowPrerelease must be true" );
        }

        return version.GetString( )!.Trim( );
    }

    private static void ValidateLibadwaitaFloors( Func<string, string> read, string floor, List<string> errors )
    {
        string[] expectedFeatures = ["v" + floor.Replace( '.', '_' )];
        var features = ReadLibadwaitaFeatures( read( RepositoryPaths.ConfiguratorCargoManifest ), errors );
        if ( !features.SequenceEqual( expectedFeatures ) )
        {
            errors.Add( $"{RepositoryPaths.ConfiguratorCargoManifest} libadwaita features: expected {FormatList( expectedFeatures )}, " +
                        $"got {FormatList( features )}" );
        }

        foreach ( var (label, path, pattern) in LibadwaitaFloorSurfaces )
        {
            var matches = Regex.Matches( read( path ), pattern, RegexOptions.Multiline );
            if ( matches.Count != 1 )
            {
                errors.Add( $"{label}: expected one libadwaita floor, found {matches.Count}" );
                continue;
            }

            RequireEqual( errors, label, matches[0].Groups[1].Value, floor );
        }
    }

    private static string[] ReadLibadwaitaFeatures( string manifest, List<string> errors )
    {
        var dependency = LibadwaitaDependencyTable( manifest );
        if ( dependency is null )
        {
            errors.Add( $"{RepositoryPaths.ConfiguratorCargoManifest}: missing structured libadwaita dependency" );
            return [];
        }

        var features = Regex.Match( dependency, $@"(?<![\w-])features\s*=\s*\[(?<{ItemsGroup}>[^\]]*)\]" );
        if ( !features.Success )
        {
            return [];
        }

        var values = new List<string>( );
        foreach ( Match token in Regex.Matches( features.Groups[ItemsGroup].Value, TomlStringArrayToken ) )
        {
            if ( token.Groups[OtherGroup].Success )
            {
                errors.Add( $"{RepositoryPaths.ConfiguratorCargoManifest}: libadwaita features must be a string list" );
                return [];
            }
            if ( token.Groups[ValueGroup].Success )
            {
                values.Add( token.Groups[ValueGroup].Value );
            }
        }

        return [.. values];
    }

    // Accept the inline `libadwaita = { ... }` form in [dependencies] and the
    // [dependencies.libadwaita] table form; a plain version string has no features.
    private static string? LibadwaitaDependencyTable( string manifest )
    {
        var dependencies = TomlTableBody( manifest, "dependencies" );
        var inline = dependencies is null
            ? Match.Empty
            : Regex.Match( dependencies, $@"(?m)^[ \t]*libadwaita[ \t]*=[ \t]*\{{(?<{ValueGroup}>[^\n]*)\}}[ \t]*(?:#[^\n]*)?$" );
        return inline.Success ? inline.Groups[ValueGroup].Value : TomlTableBody( manifest, "dependencies.libadwaita" );
    }

    private static string? TomlTableBody( string manifest, string header )
    {
        var table = Regex.Match( manifest,
            $@"(?ms)^[ \t]*\[[ \t]*{Regex.Escape( header )}[ \t]*\][ \t]*(?:#[^\n]*)?$(?<{ValueGroup}>.*?)(?=^[ \t]*\[|\z)" );
        return table.Success ? table.Groups[ValueGroup].Value : null;
    }

    private static void ValidateReleaseRunner( string workflow, string floor, List<string> errors )
    {
        var runner = ReadWorkflowJobRunner( workflow, errors );
        if ( runner is null )
        {
            return;
        }

        if ( !ReleaseRunnerLibadwaitaFloors.TryGetValue( runner, out var runnerFloor ) )
        {
            errors.Add( $"release package runner libadwaita floor: no reviewed contract for {runner}" );
            return;
        }

        RequireEqual( errors, $"release package runner {runner} libadwaita floor", floor, runnerFloor );
    }

    private static string? ReadWorkflowJobRunner( string workflow, List<string> errors )
    {
        var start = Regex.Match( workflow, $@"(?m)^  {Regex.Escape( ReleasePackageJob )}:[ \t]*$" );
        if ( !start.Success )
        {
            errors.Add( $"{ReleaseWorkflow}: missing {ReleasePackageJob} job" );
            return null;
        }

        var remaining = workflow[(start.Index + start.Length)..];
        var nextJob = Regex.Match( remaining, @"(?m)^  [A-Za-z0-9_-]+:[ \t]*$" );
        var job = nextJob.Success ? remaining[..nextJob.Index] : remaining;
        var runners = Regex.Matches( job, @"(?m)^    runs-on:[ \t]*([^#\n]+?)[ \t]*$" );
        if ( runners.Count != 1 )
        {
            errors.Add( $"{ReleaseWorkflow} {ReleasePackageJob} job: expected one literal runs-on value, found {runners.Count}" );
            return null;
        }

        return runners[0].Groups[1].Value;
    }

    private static void ValidatePackageVersions( Func<string, string> read, string cargo, string? releaseText, List<string> errors )
    {
        var cargoLock = read( RepositoryPaths.CargoLock );
        RequireEqual( errors, "Cargo.lock wayscriber", LockVersion( cargoLock, RepositoryNames.MainPackage ), cargo );
        RequireEqual( errors, "Cargo.lock wayscriber-configurator", LockVersion( cargoLock, RepositoryNames.ConfiguratorPackage ), cargo );
        if ( !IsReleaseVersion( cargo ) )
        {
            errors.Add( $"Cargo.toml version has unsupported format: {cargo}" );
        }

        var packageBuild = read( PackageBuildPath );
        var sourceInfo = read( SourceInfoPath );
        var packageBuildVersion = ReadAssignment( packageBuild, "pkgver" );
        var sourceInfoVersion = Regex.Match( sourceInfo, @"(?m)^\s*pkgver = (.+)$" );
        var expected = ExpectedPackageVersion( cargo, releaseText, packageBuildVersion, errors );
        RequireEqual( errors, "packaging/PKGBUILD pkgver", packageBuildVersion, expected );
        RequireEqual( errors, "packaging/.SRCINFO pkgver", sourceInfoVersion.Success ? sourceInfoVersion.Groups[1].Value.Trim( ) : null,
            expected );
        RequireTemplateChecksums( errors, "packaging/PKGBUILD sha256sums", PackageBuildChecksums( packageBuild ) );
        RequireTemplateChecksums( errors, "packaging/.SRCINFO sha256sums",
            Regex.Matches( sourceInfo, @"(?m)^\s*sha256sums = (.+)$" ).Select( match => match.Groups[1].Value.Trim( ) ).ToArray( ) );
    }

    // A packaging hotfix such as 0.9.19.1 ships Cargo 0.9.19; release automation
    // names the hotfix explicitly, while a plain check accepts the recipe's hotfix.
    private static string ExpectedPackageVersion( string cargo, string? releaseText, string? packageBuildVersion, List<string> errors )
    {
        if ( releaseText is null )
        {
            return packageBuildVersion is not null && IsHotfixOf( packageBuildVersion, cargo ) ? packageBuildVersion : cargo;
        }

        if ( !IsReleaseVersion( releaseText ) )
        {
            errors.Add( $"release version has unsupported format: {releaseText}" );
        }
        else if ( releaseText != cargo && !IsHotfixOf( releaseText, cargo ) )
        {
            errors.Add( $"release version {releaseText} must equal Cargo version {cargo} or be a hotfix of it, such as {cargo}.1" );
        }

        return releaseText;
    }

    private static string[] PackageBuildChecksums( string packageBuild )
    {
        var array = Regex.Match( packageBuild, @"(?ms)^sha256sums=\((.*?)\)" );
        if ( !array.Success )
        {
            return [];
        }

        return Regex.Matches( array.Groups[1].Value, @"'([^']*)'|""([^""]*)""|(\S+)" )
            .Select( match => match.Groups.Cast<Group>( ).Skip( 1 ).First( group => group.Success ).Value.Trim( ) )
            .ToArray( );
    }

    // Repo packaging metadata is a release template; release/AUR automation writes
    // the real source archive checksum after the tag exists.
    private static void RequireTemplateChecksums( List<string> errors, string label, string[] values )
    {
        if ( values is [EnvironmentVariables.Skip] )
        {
            return;
        }

        if ( values.Length == 0 )
        {
            errors.Add( $"{label}: expected SKIP template checksum, got missing" );
            return;
        }

        var actual = string.Join( ", ", values );
        if ( values.Any( value => Regex.IsMatch( value, HashingConstants.Sha256HexPattern ) ) )
        {
            errors.Add( $"{label}: expected SKIP template checksum, got fixed SHA {actual}; " +
                        "release/AUR automation writes the real checksum after the tag exists" );
            return;
        }

        errors.Add( $"{label}: expected SKIP template checksum, got {actual}" );
    }

    private static void ValidateFlake( string flake, List<string> errors )
    {
        if ( !flake.Contains( "builtins.fromTOML (builtins.readFile ./Cargo.toml)", StringComparison.Ordinal ) )
        {
            errors.Add( "flake.nix package version should be derived from Cargo.toml" );
        }

        string[] rustVersionGate = ["package.rust-version", "rustToolchain.version", "versionAtLeast"];
        if ( !rustVersionGate.All( token => flake.Contains( token, StringComparison.Ordinal ) ) )
        {
            errors.Add( "flake.nix should compare the selected rustc against Cargo.toml rust-version" );
        }
    }

    private static void ValidateReadme( string readme, List<string> errors )
    {
        foreach ( var (pattern, label) in ReadmePinPatterns )
        {
            var pins = Regex.Matches( readme, pattern ).Select( match => match.Value ).Distinct( StringComparer.Ordinal )
                .Order( StringComparer.Ordinal );
            foreach ( var pin in pins )
            {
                errors.Add( $"README.md: {label} '{pin}' goes stale on the next release; " +
                            "use a RELEASE_TAG placeholder or link to /releases/latest" );
            }
        }
    }

    private static bool IsReleaseVersion( string value ) => ReleaseVersion.TryParse( value, out _ );

    private static bool IsHotfixOf( string value, string cargo ) =>
        ReleaseVersion.TryParse( value, out var version ) && version.IsHotfix && version.CargoVersion == cargo;

    private static string FormatList( IEnumerable<string> values ) =>
        "[" + string.Join( ", ", values.Select( value => $"'{value}'" ) ) + "]";

    private static string? LockVersion( string text, string package )
    {
        var match = Regex.Match( text,
            "(?ms)\\[\\[package\\]\\]\\s+name\\s*=\\s*\"" + Regex.Escape( package ) + "\"\\s+version\\s*=\\s*\"([^\"]+)\"" );
        return match.Success ? match.Groups[1].Value : null;
    }

    private static string? ReadAssignment( string text, string name )
    {
        var match = Regex.Match( text, $@"(?m)^{Regex.Escape( name )}=(.+)$" );
        return match.Success ? match.Groups[1].Value.Trim( ) : null;
    }

    private static void RequireEqual( List<string> errors, string label, string? actual, string expected )
    {
        if ( actual != expected )
        {
            errors.Add( $"{label}: expected {expected}, got {actual ?? "missing"}" );
        }
    }
}
