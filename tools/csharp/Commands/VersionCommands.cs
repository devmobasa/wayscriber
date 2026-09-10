using System.Text.Json;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal sealed record ReleaseVersion( int Major, int Minor, int Patch, int? Hotfix )
{
    private const string MajorGroup = "major";
    private const string MinorGroup = "minor";
    private const string PatchGroup = "patch";
    private const string HotfixGroup = "hotfix";

    public string CargoVersion => $"{Major}.{Minor}.{Patch}";
    public bool IsHotfix => Hotfix is not null;
    public ReleaseVersion NextPatch( ) => new( Major, Minor, checked(Patch + 1), null );
    public override string ToString( ) => Hotfix is null ? CargoVersion : $"{CargoVersion}.{Hotfix}";

    public static ReleaseVersion Parse( string value )
    {
        var match = Regex.Match( value,
            $@"^(?<{MajorGroup}>\d+)\.(?<{MinorGroup}>\d+)\.(?<{PatchGroup}>\d+)(?:\.(?<{HotfixGroup}>\d+))?$" );
        if ( !match.Success )
        {
            throw new ToolException( $"invalid version format: {value} (expected MAJOR.MINOR.PATCH[.HOTFIX])", ExitCodes.InvalidArguments );
        }

        return new( int.Parse( match.Groups[MajorGroup].Value ), int.Parse( match.Groups[MinorGroup].Value ),
            int.Parse( match.Groups[PatchGroup].Value ),
            match.Groups[HotfixGroup].Success ? int.Parse( match.Groups[HotfixGroup].Value ) : null );
    }
}

internal static class VersionCommands
{
    private const string WorkflowRunnerGroup = "runner";
    internal const string SupportedLibadwaitaFloor = "1.4";
    internal const string ToolSdkVersion = "11.0.100-rc.1.26425.128";
    internal const string ToolSdkRollForward = "disable";

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new(CommandAreas.Version, CommandNames.Check, "Check release and package metadata.", SideEffect.ReadOnly, Check),
        new(CommandAreas.Version, CommandNames.Bump, "Update Cargo and package versions atomically.", SideEffect.FixtureMutating, Bump),
    ];

    internal static string ReadCargoVersion( string path )
    {
        var package = Regex.Match( Files.Read( path ), @"(?ms)^\[package\]\s*(.*?)(?:^\[|\z)" );
        var version = package.Success ? Regex.Match( package.Groups[1].Value, "(?m)^version\\s*=\\s*\"([^\"]+)\"" ) : Match.Empty;
        if ( !version.Success )
        {
            throw new ToolException( $"{path}: missing [package] version" );
        }

        return version.Groups[1].Value;
    }

    private static Task<int> Check( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var releaseText = parsed.TakeOption( "--release-version" );
        parsed.RequireEmpty( "version check [--release-version X.Y.Z[.N]]" );
        var errors = Validate( context.RepositoryRoot, releaseText );
        if ( errors.Count > 0 )
        {
            throw new ToolException( "Version consistency check failed:\n" + string.Join( '\n', errors.Select( error => $"- {error}" ) ) );
        }

        var cargo = ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        var package = ReadAssignment( Files.Read( context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile ) ),
            "pkgver" )!;
        context.Output.WriteLine(
            $"Version consistency OK: Cargo={cargo}, packaging={package}, checksum=SKIP, libadwaita={SupportedLibadwaitaFloor}" );
        return Task.FromResult( ExitCodes.Success );
    }

    internal static List<string> Validate( string root, string? releaseText = null )
    {
        var errors = new List<string>( );
        string Read( params string[] parts ) => Files.Read( Path.Combine( [root, .. parts] ) );

        ValidateToolSdk( Read, errors );
        var cargo = ReadCargoVersion( Path.Combine( root, RepositoryPaths.CargoManifest ) );
        ValidateConfiguratorVersion( root, Read, cargo, errors );
        ValidatePackageVersions( Read, cargo, releaseText, errors );
        ValidateFlakeAndReadme( Read, errors );
        return errors;
    }

    private static void ValidateToolSdk( Func<string[], string> read, List<string> errors )
    {
        try
        {
            using var globalJson = JsonDocument.Parse( read( ["global.json"] ) );
            RequireEqual( errors, "global.json SDK", globalJson.RootElement.GetProperty( "sdk" ).GetProperty( "version" ).GetString( ),
                ToolSdkVersion );
            var sdk = globalJson.RootElement.GetProperty( "sdk" );
            if ( !sdk.TryGetProperty( "rollForward", out var rollForward ) || rollForward.GetString( ) != ToolSdkRollForward )
            {
                errors.Add( "global.json SDK rollForward must be disable" );
            }
            if ( !sdk.TryGetProperty( "allowPrerelease", out var allowPrerelease ) || allowPrerelease.ValueKind != JsonValueKind.True )
            {
                errors.Add( "global.json SDK allowPrerelease must be true" );
            }
        }
        catch ( Exception error ) when ( error is JsonException or KeyNotFoundException or InvalidOperationException )
        {
            errors.Add( $"global.json SDK metadata is invalid: {error.Message}" );
        }
    }

    private static void ValidateConfiguratorVersion( string root, Func<string[], string> read, string cargo, List<string> errors )
    {
        var configurator = ReadCargoVersion( Path.Combine( root, RepositoryPaths.ConfiguratorCargoManifest ) );
        RequireEqual( errors, RepositoryPaths.ConfiguratorCargoManifest, configurator, cargo );
        var configManifest = read( [RepositoryPaths.ConfiguratorCargoManifest] );
        var featureMatch = Regex.Match( configManifest, @"(?m)^libadwaita\s*=\s*\{[^\n]*features\s*=\s*\[([^]]*)\]" );
        var features = featureMatch.Success
            ? Regex.Matches( featureMatch.Groups[1].Value, "\"([^\"]+)\"" ).Select( item => item.Groups[1].Value ).ToArray( )
            : [];
        var expectedFeature = "v" + SupportedLibadwaitaFloor.Replace( '.', '_' );
        if ( !features.SequenceEqual( [expectedFeature] ) )
        {
            errors.Add(
                $"configurator/Cargo.toml libadwaita features: expected ['{expectedFeature}'], got [{string.Join( ", ", features )}]" );
        }

        var floors = new Dictionary<string, string?>
        {
            ["configurator deb libadwaita floor"] =
                MatchOne( read( [RepositoryPaths.PackagingDirectory, "package.configurator.yaml"] ),
                    @"(?m)^\s*-\s*libadwaita-1-0 \(>= ([0-9]+\.[0-9]+)\)\s*$", errors, "configurator deb libadwaita floor" ),
            ["configurator rpm libadwaita floor"] =
                MatchOne( read( [RepositoryPaths.PackagingDirectory, "package.configurator.yaml"] ),
                    @"(?m)^\s*-\s*libadwaita >= ([0-9]+\.[0-9]+)\s*$", errors, "configurator rpm libadwaita floor" ),
            ["packaging/PKGBUILD libadwaita floor"] =
                MatchOne( read( [RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile] ),
                    @"(?m)^\s*'libadwaita>=([0-9]+\.[0-9]+)'\s*$", errors, "packaging/PKGBUILD libadwaita floor" ),
            ["packaging/.SRCINFO libadwaita floor"] =
                MatchOne( read( [RepositoryPaths.PackagingDirectory, RepositoryNames.SourceInfoFile] ),
                    @"(?m)^\s*depends = libadwaita>=([0-9]+\.[0-9]+)\s*$", errors, "packaging/.SRCINFO libadwaita floor" ),
        };
        foreach ( var pair in floors.Where( pair => pair.Value is not null ) )
        {
            RequireEqual( errors, pair.Key, pair.Value, SupportedLibadwaitaFloor );
        }

        var workflow = read( [".github", "workflows", "build-packages.yml"] );
        var job = Regex.Match( workflow, @"(?ms)^  package:\s*$.*?(?=^  [A-Za-z0-9_-]+:\s*$|\z)" );
        var runner = job.Success ? Regex.Match( job.Value, $@"(?m)^    runs-on:\s*(?<{WorkflowRunnerGroup}>[^#\n]+?)\s*$" ) : Match.Empty;
        if ( !runner.Success )
        {
            errors.Add( ".github/workflows/build-packages.yml package job: expected one literal runs-on value" );
        }
        else if ( runner.Groups[WorkflowRunnerGroup].Value != PackagingPlatform.UbuntuRunner )
        {
            errors.Add( $"release package runner libadwaita floor: no reviewed contract for {runner.Groups[WorkflowRunnerGroup].Value}" );
        }
    }

    private static void ValidatePackageVersions( Func<string[], string> read, string cargo, string? releaseText, List<string> errors )
    {
        RequireEqual( errors, "Cargo.lock wayscriber", LockVersion( read( [RepositoryPaths.CargoLock] ), RepositoryNames.MainPackage ),
            cargo );
        RequireEqual( errors, "Cargo.lock wayscriber-configurator",
            LockVersion( read( [RepositoryPaths.CargoLock] ), RepositoryNames.ConfiguratorPackage ), cargo );
        var baseVersion = ReleaseVersion.Parse( cargo );
        ReleaseVersion? release = releaseText is null ? null : ReleaseVersion.Parse( releaseText );
        if ( release is not null && release.CargoVersion != cargo )
        {
            errors.Add( $"release version {release} must equal Cargo version {cargo} or be a hotfix of it, such as {cargo}.1" );
        }

        var pkgbuildVersion = ReadAssignment( read( [RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile] ), "pkgver" );
        var srcinfoVersion = Regex
            .Match( read( [RepositoryPaths.PackagingDirectory, RepositoryNames.SourceInfoFile] ), @"(?m)^\s*pkgver = (.+)$" ).Groups[1]
            .Value.Trim( );
        var expectedPackage = release?.ToString( ) ??
                              (pkgbuildVersion is not null &&
                               Regex.IsMatch( pkgbuildVersion, $@"^{Regex.Escape( baseVersion.CargoVersion )}\.\d+$" )
                                  ? pkgbuildVersion
                                  : cargo);
        RequireEqual( errors, "packaging/PKGBUILD pkgver", pkgbuildVersion, expectedPackage );
        RequireEqual( errors, "packaging/.SRCINFO pkgver", srcinfoVersion, expectedPackage );
        CheckTemplateChecksum( errors, "packaging/PKGBUILD sha256sums",
            Regex.Match( read( [RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile] ), @"(?ms)^sha256sums=\((.*?)\)" )
                .Groups[1].Value );
        CheckTemplateChecksum( errors, "packaging/.SRCINFO sha256sums",
            string.Join( ' ',
                Regex.Matches( read( [RepositoryPaths.PackagingDirectory, RepositoryNames.SourceInfoFile] ), @"(?m)^\s*sha256sums = (.+)$" )
                    .Select( item => item.Groups[1].Value ) ) );
    }

    private static void ValidateFlakeAndReadme( Func<string[], string> read, List<string> errors )
    {
        var flake = read( ["flake.nix"] );
        if ( !flake.Contains( "builtins.fromTOML (builtins.readFile ./Cargo.toml)", StringComparison.Ordinal ) )
        {
            errors.Add( "flake.nix package version should be derived from Cargo.toml" );
        }

        if ( !(flake.Contains( "package.rust-version" ) && flake.Contains( "rustToolchain.version" ) &&
               flake.Contains( "versionAtLeast" )) )
        {
            errors.Add( "flake.nix should compare selected rustc against Cargo.toml rust-version" );
        }

        var readme = read( ["README.md"] );
        foreach ( Match match in Regex.Matches( readme,
                     @"wayscriber\?ref=v?\d+\.\d+\.\d+(?:\.\d+)?|/releases/(?:tag|download)/v?\d+\.\d+\.\d+(?:\.\d+)?" ) )
        {
            errors.Add( $"README.md: pinned release reference '{match.Value}' goes stale on the next release" );
        }
    }

    private static async Task<int> Bump( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var dryRun = parsed.TakeFlag( "--dry-run" );
        if ( parsed.Remaining.Count > 1 )
        {
            throw new ToolException( "version bump [--dry-run] [X.Y.Z[.N]]", ExitCodes.InvalidArguments );
        }

        var current = ReleaseVersion.Parse( ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) ) );
        var next = parsed.Remaining.Count == 1 ? ReleaseVersion.Parse( parsed.Remaining[0] ) : current.NextPatch( );
        await context.Output.WriteLineAsync( $"Current version: {current.CargoVersion}" );
        await context.Output.WriteLineAsync( $"Bumping to:      {next}" );
        if ( next.IsHotfix )
        {
            await context.Output.WriteLineAsync( $"Cargo version:   {next.CargoVersion} (release version has hotfix)" );
        }

        if ( File.Exists( context.Path( RepositoryPaths.CargoLock ) ) )
        {
            await context.Run( Programs.Cargo, ["update", CommandLineOptions.Workspace, "--offline", "--dry-run"], capture: true );
        }

        var outputs = new AtomicFileSet( );
        foreach ( var relative in new[] { RepositoryPaths.CargoManifest, RepositoryPaths.ConfiguratorCargoManifest } )
        {
            var path = context.Path( relative.Split( '/' ) );
            outputs.Add( path,
                Files.ReplaceSingle( Files.Read( path ), """(?ms)(^\[package\]\s*.*?^version\s*=\s*")[^"]+(")""",
                    $"${{1}}{next.CargoVersion}$2", relative ) );
        }

        var pkgbuildPath = context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile );
        var pkgbuild = Files.ReplaceSingle( Files.Read( pkgbuildPath ), @"(?m)^pkgver=.*$", $"pkgver={next}", "packaging/PKGBUILD pkgver" );
        pkgbuild = Files.ReplaceSingle( pkgbuild, "(?m)^sha256sums=.*$", "sha256sums=('SKIP')", "packaging/PKGBUILD checksum" );
        outputs.Add( pkgbuildPath, pkgbuild );
        if ( dryRun )
        {
            await context.Output.WriteLineAsync(
                "dry-run: would update Cargo.toml, configurator/Cargo.toml, Cargo.lock, packaging/PKGBUILD, and packaging/.SRCINFO" );
            await context.Output.WriteLineAsync( "Dry run complete (no changes made)" );
            return ExitCodes.Success;
        }

        var paths = new[]
        {
            context.Path( RepositoryPaths.CargoManifest ), context.Path( RepositoryPaths.ConfiguratorCargoManifest ),
            context.Path( RepositoryPaths.CargoLock ), pkgbuildPath,
            context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.SourceInfoFile )
        };
        var snapshots = paths.ToDictionary( path => path, path => File.Exists( path ) ? File.ReadAllBytes( path ) : null,
            StringComparer.Ordinal );
        try
        {
            outputs.Commit( );
            if ( File.Exists( context.Path( RepositoryPaths.CargoLock ) ) )
            {
                await context.Run( Programs.Cargo, ["update", CommandLineOptions.Workspace, "--offline"] );
            }

            var srcinfo = await context.Run( Programs.Makepkg, ["--printsrcinfo"], context.Path( RepositoryPaths.PackagingDirectory ),
                capture: true );
            Files.WriteAtomic( context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.SourceInfoFile ), srcinfo.StandardOutput );
            var errors = Validate( context.RepositoryRoot, next.ToString( ) );
            if ( errors.Count > 0 )
            {
                throw new ToolException( string.Join( '\n', errors ) );
            }
        }
        catch
        {
            foreach ( var pair in snapshots )
            {
                if ( pair.Value is null )
                {
                    File.Delete( pair.Key );
                }
                else
                {
                    await File.WriteAllBytesAsync( pair.Key, pair.Value );
                }
            }

            throw;
        }

        await context.Output.WriteLineAsync( $"Updated versions to {next}" );
        return ExitCodes.Success;
    }

    private static string? MatchOne( string text, string pattern, List<string> errors, string label )
    {
        var matches = Regex.Matches( text, pattern );
        if ( matches.Count != 1 )
        {
            errors.Add( $"{label}: expected one libadwaita floor, found {matches.Count}" );
            return null;
        }

        return matches[0].Groups[1].Value;
    }

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

    private static void CheckTemplateChecksum( List<string> errors, string label, string value )
    {
        var values = Regex.Matches( value, @"[A-Za-z0-9]+" ).Select( match => match.Value ).ToArray( );
        if ( !values.SequenceEqual( [EnvironmentVariables.Skip] ) )
        {
            errors.Add( $"{label}: expected SKIP template checksum, got {(values.Length == 0 ? "missing" : string.Join( ", ", values ))}" );
        }
    }
}
