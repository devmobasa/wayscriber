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
    public ReleaseVersion NextPatch( )
    {
        if ( Patch == int.MaxValue )
        {
            throw new ToolException( $"cannot increment the patch version of {CargoVersion}; pass the next version explicitly",
                ExitCodes.InvalidArguments );
        }

        return new( Major, Minor, Patch + 1, null );
    }
    public override string ToString( ) => Hotfix is null ? CargoVersion : $"{CargoVersion}.{Hotfix}";

    // ASCII digits without leading zeros, as SemVer numbers are, so a parsed
    // version prints back as the text it came from.
    private const string Number = "0|[1-9][0-9]*";

    public static ReleaseVersion Parse( string value )
    {
        if ( !TryParse( value, out var version ) )
        {
            throw new ToolException( $"invalid version format: {value} (expected MAJOR.MINOR.PATCH[.HOTFIX])", ExitCodes.InvalidArguments );
        }

        return version;
    }

    public static bool TryParse( string value, [System.Diagnostics.CodeAnalysis.NotNullWhen( true )] out ReleaseVersion? version )
    {
        version = null;
        var match = Regex.Match( value,
            $@"^(?<{MajorGroup}>{Number})\.(?<{MinorGroup}>{Number})\.(?<{PatchGroup}>{Number})(?:\.(?<{HotfixGroup}>{Number}))?\z" );
        if ( !match.Success ||
             !int.TryParse( match.Groups[MajorGroup].Value, out var major ) ||
             !int.TryParse( match.Groups[MinorGroup].Value, out var minor ) ||
             !int.TryParse( match.Groups[PatchGroup].Value, out var patch ) )
        {
            return false;
        }

        int? hotfix = null;
        if ( match.Groups[HotfixGroup].Success )
        {
            if ( !int.TryParse( match.Groups[HotfixGroup].Value, out var number ) )
            {
                return false;
            }
            hotfix = number;
        }

        version = new( major, minor, patch, hotfix );
        return true;
    }
}

internal static partial class VersionCommands
{
    private const string OfflineResolutionFailure =
        "cannot resolve locked dependencies offline; run `dotnet run tools/wayscriber.cs --no-build -- dev fetch` " +
        "before bumping the version. No version files changed.";

    // Raise this floor only as a coordinated release-platform change.
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
        EnsureConsistent( context.RepositoryRoot, releaseText );

        var cargo = ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        var package = ReadAssignment( Files.Read( context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile ) ),
            "pkgver" )!;
        context.Output.WriteLine(
            $"Version consistency OK: Cargo={cargo}, packaging={package}, checksum=SKIP, libadwaita={SupportedLibadwaitaFloor}" );
        return Task.FromResult( ExitCodes.Success );
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

        // Resolve against the existing manifests without writing the lockfile. An empty
        // dependency cache must fail before either manifest or any package metadata changes.
        if ( File.Exists( context.Path( RepositoryPaths.CargoLock ) ) )
        {
            await RequireOfflineResolution( context );
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
            EnsureConsistent( context.RepositoryRoot, next.ToString( ) );
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

    private static async Task RequireOfflineResolution( ToolContext context )
    {
        try
        {
            await context.Run( Programs.Cargo, ["update", CommandLineOptions.Workspace, "--offline", "--dry-run"], capture: true );
        }
        catch ( ToolException error ) when ( error.ExitCode != ExitCodes.CommandNotFound )
        {
            throw new ToolException( OfflineResolutionFailure );
        }
    }
}
