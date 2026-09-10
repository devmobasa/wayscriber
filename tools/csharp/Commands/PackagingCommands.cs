using System.IO.Compression;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal static class PackagingCommands
{
    private const string PublicKeyFileName = "WAYSCRIBER-GPG-KEY.asc";
    private const int UnixExitCodeCount = 256;
    private const int UnixPermissionBitsMask = 0x1ff;
    private const int OctalNumberBase = 8;
    private const int UnixModeDigitCount = 4;
    private const int MinimumInstallerManifestBodyLines = 4;
    private const int BytesPerMebibyte = 1024 * 1024;
    private const int MaximumInstallerMebibytes = 1;
    private const int MaximumInstallerBytes = MaximumInstallerMebibytes * BytesPerMebibyte;
    private const int InstallerDownloadTimeoutMinutes = 1;
    private const int NfpmDownloadTimeoutMinutes = 2;
    private const int GpgSecretKeyIdFieldIndex = 4;
    private const string DefaultNfpmVersion = "2.43.4";
    private const string InstallerManifestBeginMarker = "# ARCH_INSTALL_MANIFEST_BEGIN";
    private const string InstallerManifestEndMarker = "# ARCH_INSTALL_MANIFEST_END";
    private const string InstallerManifestFunction = "release_manifest() {";
    private const string InstallerManifestPrint = "printf '%s\\n' \\";
    private const string ReleaseArchiveRootPattern = @"^wayscriber-v\d+\.\d+\.\d+(?:\.\d+)?-linux-x86_64$";
    private const string ReleaseArchivePathPattern = @"^[-A-Za-z0-9._/+]+$";
    private const string ValidServiceCommandPattern = """^ExecStart=(?:")?/usr/bin/wayscriber(?:")? --daemon$""";
    private const string SystemdExecDirectivePattern = @"^\s*Exec[A-Za-z]*=";
    private static readonly int[] ConfiguratorIconSizes = [24, 64, 128];
    private static readonly int[] WayscriberIconSizes = [16, 19, 22, 24, 38, 64, 128];

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Package, CommandNames.Build, "Build tar, deb, and rpm artifacts.", SideEffect.FixtureMutating, Build ),
        new( CommandAreas.Package, CommandNames.CheckArchInstaller, "Compare an installer manifest with a tarball.", SideEffect.ReadOnly, CheckArchInstaller ),
        new( CommandAreas.Package, CommandNames.CheckLiveArchInstaller, "Download and check the deployed Arch installer.", SideEffect.ReadOnly, CheckLiveArchInstaller ),
        new( CommandAreas.Package, CommandNames.VerifyArtifacts, "Verify release artifact metadata and layouts.", SideEffect.ReadOnly, VerifyArtifacts ),
        new( CommandAreas.Package, CommandNames.SmokeUbuntu, "Install packages in a disposable Ubuntu container.", SideEffect.MachineMutating, SmokeUbuntu ),
        new( CommandAreas.Package, CommandNames.BuildRepositories, "Build apt and rpm repositories.", SideEffect.FixtureMutating, BuildRepositories ),
        new( CommandAreas.ContinuousIntegration, CommandNames.InstallNfpm, "Install a pinned nfpm binary.", SideEffect.MachineMutating, InstallNfpm ),
    ];

    private static async Task<int> Build( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var version = parsed.TakeOption( CommandLineOptions.Version ) ?? context.Environment( EnvironmentVariables.Version ) ?? VersionCommands.ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        _ = ReleaseVersion.Parse( version );
        var defaultFormats = $"{PackageFormats.Tar},{PackageFormats.Debian},{PackageFormats.Rpm}";
        var formats = (parsed.TakeOption( "--formats" ) ?? context.Environment( EnvironmentVariables.Formats ) ?? defaultFormats)
            .Split( ',', StringSplitOptions.RemoveEmptyEntries );
        var artifactRoot = Path.GetFullPath( parsed.TakeOption( "--artifact-root" ) ?? context.Environment( EnvironmentVariables.ArtifactRoot ) ?? context.Path( "dist" ), context.RepositoryRoot );
        var strip = !parsed.TakeFlag( "--no-strip" );
        _ = parsed.TakeFlag( "--strip" );
        var skipBuild = parsed.TakeFlag( "--skip-build" ) || context.Environment( EnvironmentVariables.SkipBuild ) == EnvironmentVariables.Enabled;
        var configurator = !parsed.TakeFlag( "--no-configurator" ) &&
            context.Environment( EnvironmentVariables.PackageConfigurator ) != EnvironmentVariables.Disabled;
        parsed.RequireEmpty( "package build [--version VERSION] [--formats tar,deb,rpm] [--artifact-root PATH] [--no-strip] [--skip-build] [--no-configurator]" );
        var gtkPrefix = context.Environment( EnvironmentVariables.Gtk4LayerShellPrefix ) ??
            context.Path( RepositoryPaths.TargetDirectory, "release-deps", RepositoryNames.Gtk4LayerShell );
        var environment = new Dictionary<string, string?>
        {
            [EnvironmentVariables.Version] = version,
            [EnvironmentVariables.WayscriberReleaseVersion] = version,
            [EnvironmentVariables.PackageConfigPath] = Path.Combine( gtkPrefix, "lib/pkgconfig" ) + (context.Environment( EnvironmentVariables.PackageConfigPath ) is { Length: > 0 } existing ? $":{existing}" : string.Empty),
            [EnvironmentVariables.SystemGtk4LayerShellLink] = NativeLibraryModes.Static,
        };
        Directory.CreateDirectory( artifactRoot );
        if ( !skipBuild )
        {
            await ToolApplication.RunNestedAsync( context, CommandAreas.Native, CommandNames.InstallGtk4LayerShell,
                ["--prefix", gtkPrefix, "--library-mode", NativeLibraryModes.Static] );
            await context.Run( Programs.Cargo, ["build", CommandLineOptions.Locked, CommandLineOptions.Release, CommandLineOptions.Binaries], environment: environment );
            if ( configurator )
            {
                await context.Run( Programs.Cargo, ["build", CommandLineOptions.Locked, CommandLineOptions.Release, CommandLineOptions.Binaries, "--manifest-path", RepositoryPaths.ConfiguratorCargoManifest], environment: environment );
            }
        }
        var mainBinary = context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.MainPackage );
        var configBinary = context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.ConfiguratorPackage );
        await ToolApplication.RunNestedAsync( context, CommandAreas.Elf, CommandNames.VerifyStatic, [mainBinary] );
        await VerifyGlibc( context, mainBinary );
        if ( configurator )
        {
            if ( !File.Exists( configBinary ) )
            {
                throw new ToolException( $"Missing release binary: {configBinary}" );
            }
            await VerifyGlibc( context, configBinary );
        }
        if ( strip )
        {
            await StripBinaries( context, mainBinary, configBinary, configurator );
        }

        var artifacts = await BuildArtifacts( context, formats, artifactRoot, version, configurator, environment );
        WriteManifest( artifactRoot, version, artifacts );
        return ExitCodes.Success;
    }

    private static async Task StripBinaries( ToolContext context, string mainBinary, string configuratorBinary, bool includeConfigurator )
    {
        var available = await context.Run( Programs.Which, [Programs.Strip], capture: true, trace: false,
            allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( available.ExitCode != ExitCodes.Success )
        {
            await context.Error.WriteLineAsync( "[WARN] 'strip' not found; binaries will remain unstripped" );
            return;
        }

        var nonFatalExitCodes = Enumerable.Range( 0, UnixExitCodeCount ).ToHashSet( );
        var mainStrip = await context.Run( Programs.Strip, [mainBinary], allowedExitCodes: nonFatalExitCodes );
        if ( mainStrip.ExitCode != ExitCodes.Success )
        {
            await context.Error.WriteLineAsync( "[WARN] strip failed for wayscriber" );
        }
        if ( includeConfigurator )
        {
            var configuratorStrip = await context.Run( Programs.Strip, [configuratorBinary], allowedExitCodes: nonFatalExitCodes );
            if ( configuratorStrip.ExitCode != ExitCodes.Success )
            {
                await context.Error.WriteLineAsync( "[WARN] strip failed for configurator" );
            }
        }

        await ToolApplication.RunNestedAsync( context, CommandAreas.Elf, CommandNames.VerifyStatic, [mainBinary] );
    }

    private static async Task<List<string>> BuildArtifacts( ToolContext context, IEnumerable<string> formats, string artifactRoot,
        string version, bool includeConfigurator, IReadOnlyDictionary<string, string?> environment )
    {
        var artifacts = new List<string>( );
        foreach ( var format in formats )
        {
            if ( format == PackageFormats.Tar )
            {
                artifacts.Add( await BuildTar( context, artifactRoot, version, configurator: false ) );
                if ( includeConfigurator )
                {
                    artifacts.Add( await BuildTar( context, artifactRoot, version, configurator: true ) );
                }
                continue;
            }

            if ( format is PackageFormats.Debian or PackageFormats.Rpm )
            {
                artifacts.Add( await BuildNfpm( context, artifactRoot, format, configurator: false, environment ) );
                if ( includeConfigurator )
                {
                    artifacts.Add( await BuildNfpm( context, artifactRoot, format, configurator: true, environment ) );
                }
                continue;
            }

            await context.Error.WriteLineAsync( $"Unknown format '{format}', skipping" );
        }

        return artifacts;
    }

    private static async Task VerifyGlibc( ToolContext context, string binary )
    {
        if ( !File.Exists( binary ) )
        {
            throw new ToolException( $"Missing release binary: {binary}" );
        }
        var result = await context.Run( Programs.ReadElf, ["--version-info", binary], capture: true );
        var versions = Regex.Matches( result.StandardOutput, @"GLIBC_([0-9]+(?:\.[0-9]+)+)" ).Select( match => Version.Parse( match.Groups[1].Value ) ).ToArray( );
        if ( versions.Length == 0 )
        {
            throw new ToolException( $"Could not determine glibc requirement for {binary}" );
        }
        var maximum = versions.Max( )!;
        if ( maximum > Version.Parse( PackagingPlatform.MaximumGlibcVersion ) )
        {
            throw new ToolException( $"{binary} requires GLIBC_{maximum}; release floor is GLIBC_{PackagingPlatform.MaximumGlibcVersion}" );
        }
        await context.Error.WriteLineAsync(
            $"[INFO] Verified {Path.GetFileName( binary )} requires at most GLIBC_{PackagingPlatform.MaximumGlibcVersion} (found GLIBC_{maximum})" );
    }

    private static async Task<string> BuildTar( ToolContext context, string output, string version, bool configurator )
    {
        var name = configurator ? $"wayscriber-configurator-v{version}-linux-x86_64" : $"wayscriber-v{version}-linux-x86_64";
        var root = Path.Combine( output, name );
        if ( Directory.Exists( root ) )
        {
            Directory.Delete( root, recursive: true );
        }
        Directory.CreateDirectory( root );
        try
        {
            void Copy( string source, string relative, UnixFileMode mode )
            {
                var destination = Path.Combine( root, relative );
                Directory.CreateDirectory( Path.GetDirectoryName( destination )! );
                File.Copy( source, destination, overwrite: true );
                if ( !OperatingSystem.IsWindows( ) )
                {
                    File.SetUnixFileMode( destination, mode );
                }
            }
            var regular = UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.GroupRead | UnixFileMode.OtherRead;
            var executable = regular | UnixFileMode.UserExecute | UnixFileMode.GroupExecute | UnixFileMode.OtherExecute;
            if ( configurator )
            {
                Copy( context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.ConfiguratorPackage ),
                    "usr/bin/wayscriber-configurator", executable );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "wayscriber-configurator.desktop" ), "usr/share/applications/wayscriber-configurator.desktop", regular );
                foreach ( var size in ConfiguratorIconSizes )
                {
                    Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", $"wayscriber-configurator-{size}.png" ), $"usr/share/icons/hicolor/{size}x{size}/apps/wayscriber-configurator.png", regular );
                }
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber-configurator.svg" ), "usr/share/icons/hicolor/scalable/apps/wayscriber-configurator.svg", regular );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber-configurator-128.png" ), "usr/share/pixmaps/wayscriber-configurator.png", regular );
                Copy( context.Path( "README.md" ), "usr/share/doc/wayscriber-configurator/README.md", regular );
                Copy( context.Path( "LICENSE" ), "usr/share/doc/wayscriber-configurator/LICENSE", regular );
            }
            else
            {
                Copy( context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.MainPackage ),
                    "usr/bin/wayscriber", executable );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.UserServiceFile ), "usr/lib/systemd/user/wayscriber.service", regular );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "wayscriber.desktop" ), "usr/share/applications/wayscriber.desktop", regular );
                foreach ( var size in WayscriberIconSizes )
                {
                    Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", $"wayscriber-{size}.png" ), $"usr/share/icons/hicolor/{size}x{size}/apps/wayscriber.png", regular );
                    Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", $"wayscriber-{size}.png" ), $"usr/share/icons/hicolor/{size}x{size}/status/wayscriber.png", regular );
                }
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber.svg" ), "usr/share/icons/hicolor/scalable/apps/wayscriber.svg", regular );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber-symbolic.svg" ), "usr/share/icons/hicolor/symbolic/apps/wayscriber-symbolic.svg", regular );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber-128.png" ), "usr/share/pixmaps/wayscriber.png", regular );
                Copy( context.Path( "README.md" ), "usr/share/doc/wayscriber/README.md", regular );
                Copy( context.Path( "config.example.toml" ), "usr/share/doc/wayscriber/config.example.toml", regular );
                Copy( context.Path( "LICENSE" ), "usr/share/doc/wayscriber/LICENSE", regular );
                Copy( context.Path( "LICENSE" ), "usr/share/licenses/wayscriber/LICENSE", regular );
                Copy( context.Path( RepositoryPaths.PackagingDirectory, "licenses", "gtk4-layer-shell.LICENSE" ), "usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell", regular );
            }
            var tarball = Path.Combine( output, name + ".tar.gz" );
            File.Delete( tarball );
            await context.Run( Programs.Tar, [CommandLineOptions.ChangeDirectory, output, "-czf", tarball, name] );
            if ( !File.Exists( tarball ) )
            {
                throw new ToolException( $"tar did not create {tarball}" );
            }
            return tarball;
        }
        finally
        {
            if ( Directory.Exists( root ) )
            {
                Directory.Delete( root, recursive: true );
            }
        }
    }

    private static async Task<string> BuildNfpm( ToolContext context, string output, string format, bool configurator,
        IReadOnlyDictionary<string, string?> environment )
    {
        var prefix = configurator ? RepositoryNames.ConfiguratorPackage : RepositoryNames.MainPackage;
        var architecture = format == PackageFormats.Debian ? "amd64" : "x86_64";
        var target = Path.Combine( output, $"{prefix}-{architecture}.{format}" );
        var configVariable = configurator ? EnvironmentVariables.NfpmConfiguratorConfig : EnvironmentVariables.NfpmMainConfig;
        var config = context.Environment( configVariable ) ??
            context.Path( RepositoryPaths.PackagingDirectory, configurator ? "package.configurator.yaml" : "package.wayscriber.yaml" );
        File.Delete( target );
        await context.Run( Programs.Nfpm, ["pkg", "--packager", format, "--config", config, "--target", target], environment: environment );
        if ( !File.Exists( target ) )
        {
            throw new ToolException( $"nfpm did not create {target}" );
        }
        return target;
    }

    private static void WriteManifest( string output, string version, IReadOnlyList<string> artifacts )
    {
        var entries = artifacts.Select( path => new { name = Path.GetFileName( path ), sha256 = Files.Sha256( path ), size = new FileInfo( path ).Length } ).ToArray( );
        Files.WriteAtomic( Path.Combine( output, "checksums.txt" ), string.Concat( entries.Select( entry => $"{entry.sha256}  {entry.name}\n" ) ) );
        Files.WriteAtomic( Path.Combine( output, "manifest.json" ), JsonSerializer.Serialize( new
        {
            version,
            artifacts = entries
        }, new JsonSerializerOptions { WriteIndented = true } ) + "\n" );
    }

    private static async Task<int> CheckArchInstaller( ToolContext context, string[] args )
    {
        if ( OperatingSystem.IsWindows( ) )
        {
            throw new ToolException( "Arch installer checks require Linux." );
        }
        var parsed = new Arguments( args );
        var installer = Path.GetFullPath( parsed.TakeOption( "--installer", required: true )!, Environment.CurrentDirectory );
        var archive = Path.GetFullPath( parsed.TakeOption( "--archive", required: true )!, Environment.CurrentDirectory );
        parsed.RequireEmpty( "package check-arch-installer --installer FILE --archive FILE" );
        if ( !File.Exists( installer ) || !File.Exists( archive ) )
        {
            throw new ToolException( "Installer or archive was not found." );
        }
        var entries = ParseInstallerManifest( Files.Read( installer ) );
        var listing = await context.Run( Programs.Tar, ["-tzf", archive], capture: true );
        var archiveRoot = ValidateArchiveListing( listing.StandardOutput );
        using var temporary = new TemporaryDirectory( "wayscriber-arch-manifest" );
        await context.Run( Programs.Tar, ["-xzf", archive, CommandLineOptions.ChangeDirectory, temporary.Path] );
        var roots = Directory.GetDirectories( temporary.Path );
        if ( roots.Length != 1 || Path.GetFileName( roots[0] ) != archiveRoot )
        {
            throw new ToolException( "Archive has an unexpected top-level directory." );
        }

        await ValidateExtractedArchive( context, roots[0] );
        var usr = Path.Combine( roots[0], "usr" );
        if ( !Directory.Exists( usr ) )
        {
            throw new ToolException( "Archive does not contain usr/." );
        }

        var files = Directory.EnumerateFiles( usr, "*", SearchOption.AllDirectories ).Select( path => Path.GetRelativePath( usr, path ).Replace( Path.DirectorySeparatorChar, '/' ) ).Order( ).ToArray( );
        if ( !entries.Keys.Order( ).SequenceEqual( files ) )
        {
            throw new ToolException( "Installer manifest and release archive file paths differ." );
        }
        foreach ( var pair in entries )
        {
            var path = Path.Combine( usr, pair.Key );
            if ( File.ResolveLinkTarget( path, returnFinalTarget: false ) is not null )
            {
                throw new ToolException( $"Archive contains a symbolic link: {pair.Key}" );
            }
            var actual = Convert.ToString( ( int ) File.GetUnixFileMode( path ) & UnixPermissionBitsMask, OctalNumberBase )!
                .PadLeft( UnixModeDigitCount, '0' );
            if ( actual != pair.Value )
            {
                throw new ToolException( $"mode mismatch for usr/{pair.Key}: expected {pair.Value}, found {actual}" );
            }
        }
        var service = Files.Read( Path.Combine( usr, "lib/systemd/user/wayscriber.service" ) );
        var serviceDirectives = ParseSystemdExecDirectives( service );
        var wayscriberCommands = serviceDirectives.Where( line =>
            line.Contains( RepositoryNames.MainPackage, StringComparison.Ordinal ) ).ToArray( );
        var validCommands = wayscriberCommands.Count( line => Regex.IsMatch( line, ValidServiceCommandPattern ) );
        if ( validCommands != 1 || wayscriberCommands.Length != validCommands )
        {
            throw new ToolException( "Release user service is incompatible with the direct installer rewrite." );
        }
        await context.Output.WriteLineAsync( $"Arch installer manifest matches {Path.GetFileName( archive )}." );
        return ExitCodes.Success;
    }

    private static string[] ParseSystemdExecDirectives( string service )
    {
        var directives = new List<string>( );
        var current = string.Empty;
        foreach ( var physicalLine in service.Replace( "\r\n", "\n", StringComparison.Ordinal ).Split( '\n' ) )
        {
            var line = physicalLine.TrimStart( );
            if ( current.Length > 0 && IsSystemdComment( line ) )
            {
                continue;
            }

            current += line;
            if ( current.EndsWith( '\\' ) )
            {
                current = current.TrimEnd( '\\' ) + " ";
                continue;
            }

            if ( Regex.IsMatch( current, SystemdExecDirectivePattern ) )
            {
                directives.Add( current );
            }
            current = string.Empty;
        }

        if ( Regex.IsMatch( current, SystemdExecDirectivePattern ) )
        {
            directives.Add( current );
        }
        return directives.ToArray( );
    }

    private static bool IsSystemdComment( string line ) => line.StartsWith( '#' ) || line.StartsWith( ';' );

    private static string ValidateArchiveListing( string listing )
    {
        var paths = listing.Split( '\n', StringSplitOptions.RemoveEmptyEntries );
        var root = paths.Select( path => path.TrimEnd( '/' ).Split( '/', 2 )[0] ).FirstOrDefault( );
        if ( root is null || !Regex.IsMatch( root, ReleaseArchiveRootPattern ) )
        {
            throw new ToolException( $"Archive has an unexpected top-level directory: {root ?? "<none>"}." );
        }

        foreach ( var path in paths )
        {
            var isExpected = path == root || path == root + "/" || path == root + "/usr" || path == root + "/usr/" ||
                path.StartsWith( root + "/usr/", StringComparison.Ordinal );
            var isUnsafe = path.StartsWith( "/", StringComparison.Ordinal ) || path.Split( '/' ).Contains( ".." );
            if ( !isExpected || isUnsafe || !Regex.IsMatch( path, ReleaseArchivePathPattern ) )
            {
                throw new ToolException( $"Archive contains an unexpected or unsafe path: {path}" );
            }
        }

        return root;
    }

    private static async Task ValidateExtractedArchive( ToolContext context, string root )
    {
        var unsupportedType = await context.Run(
            Programs.Find,
            [root, "!", "-type", "d", "!", "-type", "f", "-print", "-quit"],
            capture: true,
            trace: false );
        if ( unsupportedType.StandardOutput.Length > 0 )
        {
            throw new ToolException( $"Archive contains a symbolic link or special file: {unsupportedType.StandardOutput.Trim( )}" );
        }

        var hardLink = await context.Run( Programs.Find, [root, "-type", "f", "-links", "+1", "-print", "-quit"], capture: true, trace: false );
        if ( hardLink.StandardOutput.Length > 0 )
        {
            throw new ToolException( $"Archive contains a hard-linked file: {hardLink.StandardOutput.Trim( )}" );
        }
    }

    internal static Dictionary<string, string> ParseInstallerManifest( string source )
    {
        var lines = source.Replace( "\r\n", "\n", StringComparison.Ordinal ).Split( '\n' );
        if ( lines.Count( line => line == InstallerManifestBeginMarker ) != 1 || lines.Count( line => line == InstallerManifestEndMarker ) != 1 )
        {
            throw new ToolException( "installer must contain one static manifest block" );
        }

        var beginIndex = Array.IndexOf( lines, InstallerManifestBeginMarker );
        var endIndex = Array.IndexOf( lines, InstallerManifestEndMarker );
        var body = endIndex > beginIndex ? lines[(beginIndex + 1)..endIndex] : [];
        if ( body.Length < MinimumInstallerManifestBodyLines || body[0].Trim( ) != InstallerManifestFunction || body[1].Trim( ) != InstallerManifestPrint ||
            body[^1].Trim( ) != "}" )
        {
            throw new ToolException( "installer manifest block is malformed or uses unsupported syntax" );
        }

        var result = new Dictionary<string, string>( StringComparer.Ordinal );
        var entries = body[2..^1];
        for ( var index = 0; index < entries.Length; index++ )
        {
            var match = Regex.Match( entries[index], @"^\s*'(?<mode>0[0-7]{3}) (?<path>[-A-Za-z0-9._/+]+)'\s*(?<continuation>\\)?\s*$" );
            var shouldContinue = index < entries.Length - 1;
            if ( !match.Success || match.Groups["continuation"].Success != shouldContinue )
            {
                throw new ToolException( $"unsupported installer manifest syntax: {entries[index].Trim( )}" );
            }

            var path = match.Groups["path"].Value;
            if ( Path.IsPathRooted( path ) || path.Split( '/' ).Contains( ".." ) || !result.TryAdd( path, match.Groups["mode"].Value ) )
            {
                throw new ToolException( $"unsafe or duplicate manifest path: {path}" );
            }
        }

        if ( result.Count == 0 )
        {
            throw new ToolException( "installer manifest is empty or malformed" );
        }

        return result;
    }

    private static async Task<int> CheckLiveArchInstaller( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var archive = parsed.TakeOption( "--archive", required: true )!;
        var url = parsed.TakeOption( "--url" ) ?? "https://wayscriber.com/arch-install.sh";
        parsed.RequireEmpty( "package check-live-arch-installer --archive FILE [--url URL]" );
        using var temporary = new TemporaryDirectory( "wayscriber-installer" );
        var installer = Path.Combine( temporary.Path, "arch-install.sh" );
        using var client = new HttpClient { Timeout = TimeSpan.FromMinutes( InstallerDownloadTimeoutMinutes ), MaxResponseContentBufferSize = MaximumInstallerBytes };
        var bytes = await client.GetByteArrayAsync( new Uri( url ), context.CancellationToken );
        if ( bytes.Length > MaximumInstallerBytes )
        {
            throw new ToolException( $"Installer exceeds the {MaximumInstallerMebibytes} MiB limit." );
        }
        await File.WriteAllBytesAsync( installer, bytes, context.CancellationToken );
        return await CheckArchInstaller( context, ["--installer", installer, "--archive", archive] );
    }

    private static async Task<int> VerifyArtifacts( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var root = Path.GetFullPath( parsed.TakeOption( "--artifact-root" ) ?? context.Environment( EnvironmentVariables.ArtifactRoot ) ?? "dist", context.RepositoryRoot );
        var version = parsed.TakeOption( CommandLineOptions.Version ) ?? context.Environment( EnvironmentVariables.Version ) ?? VersionCommands.ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        parsed.RequireEmpty( "package verify-artifacts [--artifact-root PATH] [--version VERSION]" );
        var expected = version + "-1";
        foreach ( var package in new[] { "wayscriber-amd64.deb", "wayscriber-configurator-amd64.deb" } )
        {
            var result = await context.Run( Programs.DpkgDeb, ["-f", Path.Combine( root, package ), "Version"], capture: true );
            if ( result.StandardOutput.Trim( ) != expected )
            {
                throw new ToolException( $"{package} version mismatch." );
            }
        }
        var mainDepends = await context.Run( Programs.DpkgDeb, ["-f", Path.Combine( root, "wayscriber-amd64.deb" ), "Depends"], capture: true );
        RequireContains( mainDepends.StandardOutput,
            [$"libc6 (>= {PackagingPlatform.MaximumGlibcVersion})", $"libgtk-4-1 (>= {PackagingPlatform.MinimumGtkVersion})"],
            "wayscriber deb dependencies" );
        if ( mainDepends.StandardOutput.Contains( "libgtk4-layer-shell" ) )
        {
            throw new ToolException( "Main deb retains dynamic gtk4-layer-shell dependency." );
        }
        var mainDebFiles = await context.Run( Programs.DpkgDeb, ["-c", Path.Combine( root, "wayscriber-amd64.deb" )], capture: true );
        RequireContains( mainDebFiles.StandardOutput, ["/usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell"], "wayscriber deb files" );
        var configDepends = await context.Run( Programs.DpkgDeb, ["-f", Path.Combine( root, "wayscriber-configurator-amd64.deb" ), "Depends"], capture: true );
        RequireContains( configDepends.StandardOutput,
            [$"libc6 (>= {PackagingPlatform.MaximumGlibcVersion})", $"libadwaita-1-0 (>= {VersionCommands.SupportedLibadwaitaFloor})"],
            "configurator deb dependencies" );

        using var rpmDatabase = new TemporaryDirectory( "wayscriber-rpmdb" );
        foreach ( var package in new[] { "wayscriber-x86_64.rpm", "wayscriber-configurator-x86_64.rpm" } )
        {
            var result = await context.Run( Programs.Rpm, ["--dbpath", rpmDatabase.Path, "-qp", "--qf", "%{VERSION}-%{RELEASE}\\n", Path.Combine( root, package )], capture: true );
            if ( result.StandardOutput.Trim( ) != expected )
            {
                throw new ToolException( $"{package} version mismatch." );
            }
        }
        var mainRpmRequires = await context.Run( Programs.Rpm, ["--dbpath", rpmDatabase.Path, "-qp", "--requires", Path.Combine( root, "wayscriber-x86_64.rpm" )], capture: true );
        RequireLines( mainRpmRequires.StandardOutput,
            [$"glibc >= {PackagingPlatform.MaximumGlibcVersion}", $"gtk4 >= {PackagingPlatform.MinimumGtkVersion}"],
            "wayscriber rpm dependencies" );
        if ( mainRpmRequires.StandardOutput.Contains( RepositoryNames.Gtk4LayerShell, StringComparison.Ordinal ) )
        {
            throw new ToolException( "Main rpm retains dynamic gtk4-layer-shell dependency." );
        }
        var mainRpmFiles = await context.Run( Programs.Rpm, ["--dbpath", rpmDatabase.Path, "-qlp", Path.Combine( root, "wayscriber-x86_64.rpm" )], capture: true );
        RequireLines( mainRpmFiles.StandardOutput, ["/usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell"], "wayscriber rpm files" );
        var configRpmRequires = await context.Run( Programs.Rpm, ["--dbpath", rpmDatabase.Path, "-qp", "--requires", Path.Combine( root, "wayscriber-configurator-x86_64.rpm" )], capture: true );
        RequireLines( configRpmRequires.StandardOutput,
            [$"glibc >= {PackagingPlatform.MaximumGlibcVersion}", $"libadwaita >= {VersionCommands.SupportedLibadwaitaFloor}"],
            "configurator rpm dependencies" );

        var mainTar = await context.Run( Programs.Tar, ["-tzf", Path.Combine( root, $"wayscriber-v{version}-linux-x86_64.tar.gz" )], capture: true );
        RequireSuffixes( mainTar.StandardOutput, ["/usr/bin/wayscriber", "/usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell"], "wayscriber tar files" );
        var configTar = await context.Run( Programs.Tar, ["-tzf", Path.Combine( root, $"wayscriber-configurator-v{version}-linux-x86_64.tar.gz" )], capture: true );
        RequireSuffixes( configTar.StandardOutput, ["/usr/bin/wayscriber-configurator"], "configurator tar files" );
        await context.Output.WriteLineAsync( $"Verified release artifacts for {version}." );
        return ExitCodes.Success;
    }

    private static async Task<int> SmokeUbuntu( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var image = parsed.TakeOption( "--image" ) ?? PackagingPlatform.UbuntuImage;
        var root = Path.GetFullPath( parsed.TakeOption( "--artifact-root" ) ?? "dist", context.RepositoryRoot );
        parsed.RequireEmpty( "package smoke-ubuntu [--image IMAGE] [--artifact-root PATH]" );
        var name = $"wayscriber-package-smoke-{Guid.NewGuid( ):N}";
        try
        {
            await context.Run( Programs.Docker, ["create", "--name", name, "-v", $"{root}:/dist:ro", image, Programs.Sleep, "infinity"] );
            await context.Run( Programs.Docker, ["start", name] );
            await context.Run( Programs.Docker, ["exec", name, Programs.AptGet, "update"] );
            await context.Run( Programs.Docker, ["exec", "-e", "DEBIAN_FRONTEND=noninteractive", name, Programs.AptGet, "install", "-y", "/dist/wayscriber-amd64.deb", "/dist/wayscriber-configurator-amd64.deb"] );
            await context.Run( Programs.Docker, ["exec", name, RepositoryNames.MainPackage, CommandLineOptions.Version] );
            await context.Run( Programs.Docker, ["exec", name, Programs.Test, "-x", "/usr/bin/wayscriber-configurator"] );
        }
        finally
        {
            await context.Run( Programs.Docker, ["rm", "-f", name], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        }
        return ExitCodes.Success;
    }

    private static async Task<int> InstallNfpm( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var version = parsed.TakeOption( CommandLineOptions.Version ) ?? DefaultNfpmVersion;
        parsed.RequireEmpty( "ci install-nfpm [--version VERSION]" );
        using var temporary = new TemporaryDirectory( Programs.Nfpm );
        var archive = Path.Combine( temporary.Path, "nfpm.tar.gz" );
        using var client = new HttpClient { Timeout = TimeSpan.FromMinutes( NfpmDownloadTimeoutMinutes ) };
        await using ( var output = File.Create( archive ) )
        await using ( var input = await client.GetStreamAsync( $"https://github.com/goreleaser/nfpm/releases/download/v{version}/nfpm_{version}_Linux_x86_64.tar.gz", context.CancellationToken ) )
        {
            await input.CopyToAsync( output, context.CancellationToken );
        }
        await context.Run( Programs.Tar, ["-xzf", archive, CommandLineOptions.ChangeDirectory, temporary.Path, Programs.Nfpm] );
        await context.Run( Programs.Sudo, [Programs.Install, InstallArguments.ExecutableFile, Path.Combine( temporary.Path, Programs.Nfpm ), "/usr/local/bin/nfpm"] );
        await context.Run( Programs.Nfpm, [CommandLineOptions.Version] );
        return ExitCodes.Success;
    }

    private static async Task<int> BuildRepositories( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var artifacts = Path.GetFullPath( parsed.TakeOption( "--artifact-root" ) ?? context.Environment( EnvironmentVariables.ArtifactRoot ) ?? "dist", context.RepositoryRoot );
        var output = Path.GetFullPath( parsed.TakeOption( "--output-root" ) ?? context.Environment( EnvironmentVariables.OutputRoot ) ?? "dist/repos", context.RepositoryRoot );
        var suite = parsed.TakeOption( "--deb-suite" ) ?? context.Environment( EnvironmentVariables.DebSuite ) ?? "stable";
        var component = parsed.TakeOption( "--deb-component" ) ?? context.Environment( EnvironmentVariables.DebComponent ) ?? "main";
        var arch = parsed.TakeOption( "--deb-arch" ) ?? context.Environment( EnvironmentVariables.DebArchitecture ) ?? "amd64";
        var rpmArch = parsed.TakeOption( "--rpm-arch" ) ?? context.Environment( EnvironmentVariables.RpmArchitecture ) ?? "x86_64";
        parsed.RequireEmpty( "package build-repositories [--artifact-root PATH] [--output-root PATH] [--deb-suite NAME] [--deb-component NAME] [--deb-arch ARCH] [--rpm-arch ARCH]" );
        var mainDebName = $"{RepositoryNames.MainPackage}-amd64.deb";
        var configuratorDebName = $"{RepositoryNames.ConfiguratorPackage}-amd64.deb";
        var mainRpmName = $"{RepositoryNames.MainPackage}-{rpmArch}.rpm";
        var configuratorRpmName = $"{RepositoryNames.ConfiguratorPackage}-{rpmArch}.rpm";
        var mainDeb = Path.Combine( artifacts, mainDebName );
        var mainRpm = Path.Combine( artifacts, mainRpmName );
        if ( !File.Exists( mainDeb ) )
        {
            throw new ToolException( $"Missing deb package at {mainDeb}" );
        }
        if ( !File.Exists( mainRpm ) )
        {
            throw new ToolException( $"Missing rpm package at {mainRpm}" );
        }

        var outputParent = Path.GetDirectoryName( output );
        if ( outputParent is null )
        {
            throw new ToolException( $"Cannot determine output parent for {output}." );
        }

        Directory.CreateDirectory( outputParent );
        var staging = Path.Combine( outputParent, $".{Path.GetFileName( output )}.staging-{Guid.NewGuid( ):N}" );
        Directory.CreateDirectory( staging );
        try
        {
            using var signing = await RepositorySigning.Create( context, staging );
            await BuildAptRepository( context, signing, staging, artifacts, [mainDebName, configuratorDebName], suite, component, arch );
            await BuildRpmRepository( context, signing, staging, artifacts, [mainRpmName, configuratorRpmName] );
            ReplaceRepositoryOutput( staging, output, outputParent );

            await context.Error.WriteLineAsync( $"[build-repos] Repository build complete under {output}" );
            return ExitCodes.Success;
        }
        finally
        {
            if ( Directory.Exists( staging ) )
            {
                Directory.Delete( staging, recursive: true );
            }
        }
    }

    private static async Task BuildAptRepository( ToolContext context, RepositorySigning signing, string staging,
        string artifacts, IEnumerable<string> packageNames, string suite, string component, string arch )
    {
        var aptRoot = Path.Combine( staging, "apt" );
        var pool = Path.Combine( aptRoot, "pool/main/w/wayscriber" );
        var binaryDirectory = Path.Combine( aptRoot, $"dists/{suite}/{component}/binary-{arch}" );
        Directory.CreateDirectory( pool );
        Directory.CreateDirectory( binaryDirectory );
        CopyExistingPackages( artifacts, pool, packageNames );

        var packages = await context.Run( Programs.AptFileArchive, ["packages", "pool"], aptRoot, capture: true );
        var packageFile = Path.Combine( binaryDirectory, "Packages" );
        Files.WriteAtomic( packageFile, packages.StandardOutput );
        await using ( var input = File.OpenRead( packageFile ) )
        await using ( var outputStream = File.Create( packageFile + ".gz" ) )
        await using ( var gzip = new GZipStream( outputStream, CompressionLevel.SmallestSize ) )
        {
            await input.CopyToAsync( gzip, context.CancellationToken );
        }

        var release = await context.Run( Programs.AptFileArchive, ["-o", $"APT::FTPArchive::Release::Origin={context.Environment( EnvironmentVariables.RepositoryOrigin ) ?? "Wayscriber"}",
            "-o", $"APT::FTPArchive::Release::Label={context.Environment( EnvironmentVariables.RepositoryLabel ) ?? "Wayscriber"}", "-o", $"APT::FTPArchive::Release::Suite={suite}",
            "-o", $"APT::FTPArchive::Release::Codename={suite}", "-o", $"APT::FTPArchive::Release::Architectures={arch}",
            "-o", $"APT::FTPArchive::Release::Components={component}", "release", $"dists/{suite}"], aptRoot, capture: true );
        var releaseFile = Path.Combine( aptRoot, $"dists/{suite}/Release" );
        Files.WriteAtomic( releaseFile, release.StandardOutput );
        signing.CopyPublicKeys( aptRoot );
        await signing.SignFile( context, releaseFile, Path.Combine( aptRoot, $"dists/{suite}/InRelease" ), clearSign: true );
        await signing.SignFile( context, releaseFile, Path.Combine( aptRoot, $"dists/{suite}/Release.gpg" ), armor: true );
    }

    private static async Task BuildRpmRepository( ToolContext context, RepositorySigning signing, string staging,
        string artifacts, IEnumerable<string> packageNames )
    {
        var rpmRoot = Path.Combine( staging, PackageFormats.Rpm );
        Directory.CreateDirectory( rpmRoot );
        CopyExistingPackages( artifacts, rpmRoot, packageNames );
        signing.CopyRpmPublicKeys( rpmRoot );
        if ( context.Environment( EnvironmentVariables.SignRpms ) != EnvironmentVariables.Disabled )
        {
            foreach ( var rpm in Directory.EnumerateFiles( rpmRoot, "*.rpm" ) )
            {
                await signing.SignRpm( context, rpm );
            }
        }

        await context.Run( Programs.CreateRpmRepository, ["--update", rpmRoot] );
        await signing.SignFile( context, Path.Combine( rpmRoot, "repodata/repomd.xml" ), Path.Combine( rpmRoot, "repodata/repomd.xml.asc" ), armor: true );
    }

    private static void CopyExistingPackages( string sourceDirectory, string destinationDirectory, IEnumerable<string> packageNames )
    {
        foreach ( var packageName in packageNames )
        {
            var source = Path.Combine( sourceDirectory, packageName );
            if ( File.Exists( source ) )
            {
                File.Copy( source, Path.Combine( destinationDirectory, packageName ), true );
            }
        }
    }

    private static void ReplaceRepositoryOutput( string staging, string output, string outputParent )
    {
        string? backup = null;
        if ( Directory.Exists( output ) )
        {
            backup = Path.Combine( outputParent, $".{Path.GetFileName( output )}.previous-{Guid.NewGuid( ):N}" );
            Directory.Move( output, backup );
        }

        try
        {
            Directory.Move( staging, output );
        }
        catch
        {
            if ( backup is not null && !Directory.Exists( output ) )
            {
                Directory.Move( backup, output );
            }
            throw;
        }

        if ( backup is not null )
        {
            Directory.Delete( backup, recursive: true );
        }
    }

    private static void RequireContains( string text, IEnumerable<string> values, string label )
    {
        foreach ( var value in values )
        {
            if ( !text.Contains( value, StringComparison.Ordinal ) )
            {
                throw new ToolException( $"{label} lacks {value}" );
            }
        }
    }

    private static void RequireLines( string text, IEnumerable<string> values, string label )
    {
        var lines = text.Split( '\n', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries ).ToHashSet( StringComparer.Ordinal );
        foreach ( var value in values )
        {
            if ( !lines.Contains( value ) )
            {
                throw new ToolException( $"{label} lacks {value}" );
            }
        }
    }

    private static void RequireSuffixes( string text, IEnumerable<string> values, string label )
    {
        var lines = text.Split( '\n', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries );
        foreach ( var value in values )
        {
            if ( !lines.Any( line => line.EndsWith( value, StringComparison.Ordinal ) ) )
            {
                throw new ToolException( $"{label} lacks {value}" );
            }
        }
    }

    private sealed class RepositorySigning : IDisposable
    {
        private const int ParametricRpmSigningMacroVersion = 6;
        private const string RpmMacrosFileName = ".rpmmacros";
        private const string RpmPassphraseFileName = "rpm-signing-passphrase";

        private readonly TemporaryDirectory? _home;
        private readonly string? _key;
        private readonly string? _passphrase;
        private readonly string? _publicArmor;
        private readonly string? _publicBinary;
        private readonly IReadOnlyDictionary<string, string?>? _environment;

        private RepositorySigning( )
        {
        }

        private RepositorySigning( TemporaryDirectory home, string key, string? passphrase, string armor, string binary )
        {
            _home = home;
            _key = key;
            _passphrase = passphrase;
            _publicArmor = armor;
            _publicBinary = binary;
            _environment = new Dictionary<string, string?> { [EnvironmentVariables.GnuPgHome] = home.Path, [EnvironmentVariables.Home] = home.Path };
        }

        public static async Task<RepositorySigning> Create( ToolContext context, string output )
        {
            var encoded = context.Environment( EnvironmentVariables.GpgPrivateKeyBase64 );
            if ( string.IsNullOrWhiteSpace( encoded ) )
            {
                await context.Error.WriteLineAsync( "[WARN] GPG_PRIVATE_KEY_B64 is not set; repositories will be unsigned." );
                return new RepositorySigning( );
            }
            byte[] bytes;
            try
            {
                bytes = Convert.FromBase64String( encoded );
            }
            catch ( FormatException error )
            {
                throw new ToolException( $"GPG_PRIVATE_KEY_B64 is invalid: {error.Message}" );
            }
            var home = new TemporaryDirectory( "wayscriber-gpg" );
            if ( !OperatingSystem.IsWindows( ) )
            {
                File.SetUnixFileMode( home.Path, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute );
            }
            var environment = new Dictionary<string, string?> { [EnvironmentVariables.GnuPgHome] = home.Path, [EnvironmentVariables.Home] = home.Path };
            try
            {
                var privateKey = Path.Combine( home.Path, "repository-signing-key" );
                Files.WriteAtomic( privateKey, bytes, UnixFileMode.UserRead | UnixFileMode.UserWrite );
                await context.Run( Programs.Gpg, ["--batch", "--import", privateKey], environment: environment, capture: true );
                var listed = await context.Run( Programs.Gpg, ["--batch", "--list-secret-keys", "--with-colons"], environment: environment, capture: true );
                var detected = listed.StandardOutput.Split( '\n' ).Select( line => line.Split( ':' ) )
                    .FirstOrDefault( fields => fields.Length > GpgSecretKeyIdFieldIndex && fields[0] == "sec" )?[GpgSecretKeyIdFieldIndex];
                var key = context.Environment( EnvironmentVariables.GpgKeyId ) ?? detected;
                if ( string.IsNullOrWhiteSpace( key ) )
                {
                    throw new ToolException( "Failed to detect imported GPG key id." );
                }
                var armor = Path.Combine( output, PublicKeyFileName );
                var binary = Path.Combine( output, "WAYSCRIBER-GPG-KEY.gpg" );
                await context.Run( Programs.Gpg, ["--batch", "--yes", "--armor", "--output", armor, "--export", key], environment: environment );
                await context.Run( Programs.Gpg, ["--batch", "--yes", "--output", binary, "--export", key], environment: environment );
                if ( context.Environment( EnvironmentVariables.SignRpms ) != EnvironmentVariables.Disabled )
                {
                    var which = await context.Run( Programs.Which, [Programs.Gpg], environment: environment, capture: true );
                    var rpmVersion = await context.Run( Programs.Rpm, [CommandLineOptions.Version], environment: environment, capture: true );
                    var rpmMajorVersion = ParseRpmMajorVersion( rpmVersion.StandardOutput );
                    var passphraseFile = Path.Combine( home.Path, RpmPassphraseFileName );
                    Files.WriteAtomic( passphraseFile, (context.Environment( EnvironmentVariables.GpgPassphrase ) ?? string.Empty) + "\n",
                        UnixFileMode.UserRead | UnixFileMode.UserWrite );
                    var macros = CreateRpmMacros( rpmMajorVersion, key, home.Path, which.StandardOutput.Trim( ), passphraseFile );
                    Files.WriteAtomic( Path.Combine( home.Path, RpmMacrosFileName ), macros );
                }

                await context.Error.WriteLineAsync( $"[build-repos] Using GPG key: {key}" );
                return new RepositorySigning( home, key, context.Environment( EnvironmentVariables.GpgPassphrase ), armor, binary );
            }
            catch { home.Dispose( ); throw; }
        }

        internal static int ParseRpmMajorVersion( string output )
        {
            var match = Regex.Match( output, @"\b(?<major>[0-9]+)(?:\.[0-9]+)+\b" );
            if ( !match.Success || !int.TryParse( match.Groups["major"].Value, out var majorVersion ) )
            {
                throw new ToolException( $"Cannot determine RPM version from: {output.Trim( )}" );
            }

            return majorVersion;
        }

        internal static string CreateRpmMacros( int rpmMajorVersion, string key, string home, string gpgPath, string passphraseFile )
        {
            var common = $"%_signature gpg\n%_gpg_name {key}\n%_gpg_path {home}\n%__gpg {gpgPath}\n";
            if ( rpmMajorVersion >= ParametricRpmSigningMacroVersion )
            {
                return common + $"%_openpgp_sign_id {key}\n%_gpg_sign_cmd_extra_args --batch --pinentry-mode loopback --passphrase-file {passphraseFile}\n";
            }

            return common + $"%__gpg_sign_cmd %{{__gpg}} --batch --pinentry-mode loopback --passphrase-file {passphraseFile} --no-armor " +
                "--detach-sign --sign --local-user \"%{_gpg_name}\" --output %{__signature_filename} %{__plaintext_filename}\n";
        }

        public void CopyPublicKeys( string destination )
        {
            if ( _publicArmor is null || _publicBinary is null )
            {
                return;
            }
            File.Copy( _publicArmor, Path.Combine( destination, Path.GetFileName( _publicArmor ) ), true );
            File.Copy( _publicBinary, Path.Combine( destination, Path.GetFileName( _publicBinary ) ), true );
        }

        public void CopyRpmPublicKeys( string destination )
        {
            if ( _publicArmor is null )
            {
                return;
            }
            File.Copy( _publicArmor, Path.Combine( destination, "RPM-GPG-KEY-wayscriber.asc" ), true );
            File.Copy( _publicArmor, Path.Combine( destination, "RPM-GPG-KEY-wayscriber" ), true );
        }

        public async Task SignRpm( ToolContext context, string rpm )
        {
            if ( _key is null )
            {
                return;
            }
            await context.Run( Programs.RpmSign, ["--addsign", rpm], environment: _environment );
        }

        public async Task SignFile( ToolContext context, string source, string destination, bool armor = false, bool clearSign = false )
        {
            if ( _key is null )
            {
                return;
            }
            var arguments = new List<string> { "--batch", "--yes", "--pinentry-mode", "loopback", "--passphrase-fd", "0", "--local-user", _key };
            if ( clearSign )
            {
                arguments.Add( "--clearsign" );
            }
            else
            {
                arguments.Add( "--detach-sign" );
                if ( armor )
                {
                    arguments.Add( "--armor" );
                }
            }
            arguments.AddRange( ["--output", destination, source] );
            await context.Run( Programs.Gpg, arguments, environment: _environment, input: (_passphrase ?? string.Empty) + "\n" );
        }

        public void Dispose( ) => _home?.Dispose( );
    }
}
