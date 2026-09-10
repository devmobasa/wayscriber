using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal static class ReleaseAurCommands
{
    private const int SourceDownloadTimeoutMinutes = 2;
    private const string AurHost = "aur.archlinux.org";
    private const string AurKeyFileName = "aur";
    private const string AurKnownHostsFileName = "known_hosts.wayscriber-aur";

    private sealed record AurUpdateOptions( string ManifestPath, string? Version, string SourceDirectory, string BinaryDirectory,
        string ConfiguratorDirectory, string? SourceChecksum, bool NoConfigurator, bool Push, bool Interactive );

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Release, CommandNames.ResolveVersion, "Resolve a release version and optional GitHub output.", SideEffect.ReadOnly, ResolveVersion ),
        new( CommandAreas.Release, CommandNames.CreateTag, "Create a local annotated release tag.", SideEffect.FixtureMutating, CreateTag ),
        new( CommandAreas.Release, CommandNames.PublishTag, "Create and push an annotated release tag.", SideEffect.RemoteMutating, PublishTag ),
        new( CommandAreas.Aur, CommandNames.SourceChecksum, "Resolve a tagged source archive checksum.", SideEffect.ReadOnly, SourceChecksum ),
        new( CommandAreas.Aur, CommandNames.Update, "Update AUR package recipes from a manifest.", SideEffect.RemoteMutating, UpdateAur ),
        new( CommandAreas.Aur, CommandNames.PrepareSsh, "Install AUR SSH credentials from the environment.", SideEffect.MachineMutating, PrepareSsh ),
        new( CommandAreas.Aur, CommandNames.ConfigureGit, "Configure the AUR commit identity.", SideEffect.MachineMutating, ConfigureGit ),
        new( CommandAreas.Aur, CommandNames.Clone, "Clone the three Wayscriber AUR repositories.", SideEffect.RemoteMutating, CloneAur ),
        new( CommandAreas.Release, CommandNames.DeployPackageRepositories, "Deploy apt/rpm repositories over SSH.", SideEffect.RemoteMutating, DeployRepositories ),
    ];

    private static async Task<int> ResolveVersion( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var reference = parsed.TakeOption( "--ref-name" ) ?? context.Environment( EnvironmentVariables.GitHubRefName );
        var githubOutput = parsed.TakeOption( "--github-output" ) ?? context.Environment( EnvironmentVariables.GitHubOutput );
        parsed.RequireEmpty( "release resolve-version [--ref-name REF] [--github-output FILE]" );
        var version = reference is { Length: > 1 } && reference.StartsWith( 'v' ) ? reference[1..] : VersionCommands.ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        _ = ReleaseVersion.Parse( version );
        if ( githubOutput is not null )
        {
            await File.AppendAllTextAsync( githubOutput, $"version={version}\n", context.CancellationToken );
        }
        await context.Output.WriteLineAsync( $"Release version: {version}" );
        return ExitCodes.Success;
    }

    private static async Task<int> CreateTag( ToolContext context, string[] args )
    {
        var version = new Arguments( args ).SinglePositional( "release create-tag VERSION" );
        _ = ReleaseVersion.Parse( version );
        await EnsureVersionAndCleanTree( context, version );
        var tag = "v" + version;
        var exists = await context.Run( Programs.Git, ["rev-parse", "-q", "--verify", $"refs/tags/{tag}"], capture: true,
            allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( exists.ExitCode == ExitCodes.Success )
        {
            throw new ToolException( $"tag {tag} already exists" );
        }
        await context.Run( Programs.Git, ["tag", "-a", tag, "-m", $"Release {tag}"] );
        await context.Output.WriteLineAsync( $"Created tag {tag}" );
        return ExitCodes.Success;
    }

    private static async Task<int> PublishTag( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var version = parsed.TakeOption( CommandLineOptions.Version ) ?? VersionCommands.ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        var dryRun = parsed.TakeFlag( "--dry-run" );
        parsed.RequireEmpty( "release publish-tag [--version VERSION] [--dry-run]" );
        _ = ReleaseVersion.Parse( version );
        await EnsureVersionAndCleanTree( context, version );
        var tag = "v" + version;
        var local = await context.Run( Programs.Git, ["rev-parse", "-q", "--verify", $"refs/tags/{tag}"], capture: true, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( local.ExitCode == ExitCodes.Success )
        {
            throw new ToolException( $"Tag {tag} already exists locally; aborting." );
        }
        var remote = await context.Run( Programs.Git, ["ls-remote", "--tags", "origin", tag], capture: true );
        if ( remote.StandardOutput.Contains( tag, StringComparison.Ordinal ) )
        {
            throw new ToolException( $"Tag {tag} already exists on origin; aborting." );
        }
        if ( dryRun )
        {
            await context.Output.WriteLineAsync( $"[dry-run] git tag -a {tag} -m 'Release {tag}'\n[dry-run] git push origin {tag}" );
            return ExitCodes.Success;
        }
        await context.Run( Programs.Git, ["tag", "-a", tag, "-m", $"Release {tag}"] );
        try
        {
            await context.Run( Programs.Git, ["push", "origin", tag] );
        }
        catch { await context.Run( Programs.Git, ["tag", "-d", tag], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } ); throw; }
        return ExitCodes.Success;
    }

    private static async Task EnsureVersionAndCleanTree( ToolContext context, string version )
    {
        var errors = VersionCommands.Validate( context.RepositoryRoot, version );
        if ( errors.Count > 0 )
        {
            throw new ToolException( string.Join( '\n', errors ) );
        }
        var status = await context.Run( Programs.Git, ["status", "--porcelain", "--untracked-files=all"], capture: true );
        if ( status.StandardOutput.Length > 0 )
        {
            throw new ToolException( "Working tree is not clean; commit or stash changes before tagging." );
        }
    }

    private static async Task<int> SourceChecksum( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var version = parsed.TakeOption( CommandLineOptions.Version, required: true )!;
        var output = parsed.TakeOption( "--github-output" );
        parsed.RequireEmpty( "aur source-checksum --version VERSION [--github-output FILE]" );
        var checksum = await ResolveSourceChecksum( context, version, null );
        await context.Output.WriteLineAsync( checksum );
        if ( output is not null )
        {
            await File.AppendAllTextAsync( output, $"source_sha256={checksum}\n", context.CancellationToken );
        }
        return ExitCodes.Success;
    }

    private static async Task<string> ResolveSourceChecksum( ToolContext context, string version, string? supplied )
    {
        _ = ReleaseVersion.Parse( version );
        if ( supplied is not null )
        {
            if ( !Regex.IsMatch( supplied, HashingConstants.Sha256HexPattern ) )
            {
                throw new ToolException( $"Source archive checksum is not a sha256 digest: '{supplied}'" );
            }
            return supplied.ToLowerInvariant( );
        }
        using var temporary = new TemporaryDirectory( "wayscriber-source" );
        var archive = Path.Combine( temporary.Path, "source.tar.gz" );
        using var client = new HttpClient { Timeout = TimeSpan.FromMinutes( SourceDownloadTimeoutMinutes ) };
        await using ( var input = await client.GetStreamAsync( $"https://github.com/devmobasa/wayscriber/archive/refs/tags/v{version}.tar.gz", context.CancellationToken ) )
        await using ( var output = File.Create( archive ) )
        {
            await input.CopyToAsync( output, context.CancellationToken );
        }
        if ( new FileInfo( archive ).Length == 0 )
        {
            throw new ToolException( "Downloaded source archive is empty." );
        }
        return Files.Sha256( archive );
    }

    private static async Task<int> UpdateAur( ToolContext context, string[] args )
    {
        if ( args.Contains( "--interactive", StringComparer.Ordinal ) && !args.Contains( "--manifest", StringComparer.Ordinal ) )
        {
            return await UpdateAurInteractive( context, args.Where( argument => argument != "--interactive" ).ToArray( ) );
        }
        var options = ParseAurUpdateOptions( context, args );
        if ( !File.Exists( options.ManifestPath ) )
        {
            throw new ToolException( $"Manifest not found: {options.ManifestPath}" );
        }
        using var manifest = JsonDocument.Parse( Files.Read( options.ManifestPath ) );
        var manifestVersion = manifest.RootElement.TryGetProperty( "version", out var versionValue ) && versionValue.ValueKind == JsonValueKind.String ? versionValue.GetString( ) : null;
        var version = options.Version ?? manifestVersion;
        if ( version is null )
        {
            throw new ToolException( $"Manifest version must be a string: {options.ManifestPath}" );
        }

        _ = ReleaseVersion.Parse( version );
        var selections = SelectAurCheckouts( options.SourceDirectory, options.BinaryDirectory, options.ConfiguratorDirectory, options.NoConfigurator );
        await ValidateAurCheckouts( context, selections );

        var binarySha = selections.Any( item => item.Channel == PackageChannels.Binary ) ? ArtifactSha( manifest.RootElement, $"wayscriber-v{version}-linux-x86_64.tar.gz" ) : string.Empty;
        var sourceSha = selections.Any( item => item.Channel is PackageChannels.Source or PackageChannels.Configurator ) ? await ResolveSourceChecksum( context, version, options.SourceChecksum ) : string.Empty;
        var recipe = AssetsCommand.CreateRecipe( context.RepositoryRoot );
        await PreflightAurUpdates( context, selections, version, sourceSha, binarySha, recipe );
        await ApplyAurUpdates( context, selections, version, sourceSha, binarySha, recipe );

        var push = options.Push;
        if ( options.Interactive && !push )
        {
            await context.Output.WriteAsync( "Push updated AUR recipes? [y/N] " );
            var answer = Console.ReadLine( );
            push = string.Equals( answer, "y", StringComparison.OrdinalIgnoreCase ) || string.Equals( answer, "yes", StringComparison.OrdinalIgnoreCase );
        }
        if ( push )
        {
            await PublishAurUpdates( context, selections, version );
        }

        await context.Output.WriteLineAsync( "AUR recipes updated" );
        return ExitCodes.Success;
    }

    private static AurUpdateOptions ParseAurUpdateOptions( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var manifestPath = Path.GetFullPath( parsed.TakeOption( "--manifest" ) ?? context.Path( "dist", "manifest.json" ), Environment.CurrentDirectory );
        var version = parsed.TakeOption( CommandLineOptions.Version );
        var sourceDirectory = Path.GetFullPath( parsed.TakeOption( "--source-dir" ) ?? context.Environment( EnvironmentVariables.AurSourceDirectory ) ?? Path.Combine( context.RepositoryRoot, "..", "aur-wayscriber" ) );
        var binaryDirectory = Path.GetFullPath( parsed.TakeOption( "--bin-dir" ) ?? context.Environment( EnvironmentVariables.AurBinaryDirectory ) ?? Path.Combine( context.RepositoryRoot, "..", "aur-wayscriber-bin" ) );
        var configuratorDirectory = Path.GetFullPath( parsed.TakeOption( "--config-dir" ) ?? context.Environment( EnvironmentVariables.AurConfiguratorDirectory ) ?? Path.Combine( context.RepositoryRoot, "..", "aur-wayscriber-configurator" ) );
        var sourceChecksum = parsed.TakeOption( "--source-sha256" ) ?? context.Environment( EnvironmentVariables.AurSourceArchiveSha256 );
        var noConfigurator = parsed.TakeFlag( "--no-configurator" );
        var push = parsed.TakeFlag( "--push" );
        var interactive = parsed.TakeFlag( "--interactive" );
        parsed.RequireEmpty( "aur update [--version VERSION] [--manifest FILE] [--source-dir DIR] [--bin-dir DIR] [--config-dir DIR] [--no-configurator] [--source-sha256 SHA] [--push] [--interactive]" );
        return new AurUpdateOptions( manifestPath, version, sourceDirectory, binaryDirectory, configuratorDirectory, sourceChecksum, noConfigurator, push, interactive );
    }

    private static List<(string Channel, string Directory)> SelectAurCheckouts( string sourceDir, string binDir, string configDir, bool noConfigurator )
    {
        var selections = new List<(string Channel, string Directory)>( );
        if ( Directory.Exists( sourceDir ) )
        {
            selections.Add( (PackageChannels.Source, sourceDir) );
        }
        if ( Directory.Exists( binDir ) )
        {
            selections.Add( (PackageChannels.Binary, binDir) );
        }
        if ( !noConfigurator )
        {
            if ( !Directory.Exists( configDir ) )
            {
                throw new ToolException( $"wayscriber-configurator AUR clone not found: {configDir}" );
            }
            selections.Add( (PackageChannels.Configurator, configDir) );
        }

        if ( selections.Count == 0 )
        {
            throw new ToolException( "No AUR checkouts were found." );
        }
        return selections;
    }

    private static async Task ValidateAurCheckouts( ToolContext context, IEnumerable<(string Channel, string Directory)> selections )
    {
        foreach ( var selection in selections )
        {
            await ValidateAurCheckout( context, selection.Channel, selection.Directory );
        }
    }

    private static async Task PreflightAurUpdates( ToolContext context, IEnumerable<(string Channel, string Directory)> selections,
        string version, string sourceSha, string binarySha, JsonObject recipe )
    {
        using var preflight = new TemporaryDirectory( "wayscriber-aur-preflight" );
        foreach ( var selection in selections )
        {
            var copy = Path.Combine( preflight.Path, selection.Channel );
            CopyTree( selection.Directory, copy, includeGit: false );
            await TransformAur( context, selection.Channel, copy, version, sourceSha, binarySha, recipe );
            ValidateAurRecipe( selection.Channel, copy, version, ChecksumFor( selection.Channel, sourceSha, binarySha ), recipe );
        }
    }

    private static async Task ApplyAurUpdates( ToolContext context, IReadOnlyCollection<(string Channel, string Directory)> selections,
        string version, string sourceSha, string binarySha, JsonObject recipe )
    {
        var snapshots = AurMutationPaths( selections ).ToDictionary( path => path,
            path => File.Exists( path ) ? File.ReadAllBytes( path ) : null, StringComparer.Ordinal );
        try
        {
            foreach ( var selection in selections )
            {
                await TransformAur( context, selection.Channel, selection.Directory, version, sourceSha, binarySha, recipe );
                ValidateAurRecipe( selection.Channel, selection.Directory, version, ChecksumFor( selection.Channel, sourceSha, binarySha ), recipe );
            }
        }
        catch
        {
            foreach ( var snapshot in snapshots )
            {
                if ( snapshot.Value is null )
                {
                    File.Delete( snapshot.Key );
                }
                else
                {
                    File.WriteAllBytes( snapshot.Key, snapshot.Value );
                }
            }
            throw;
        }
    }

    private static IEnumerable<string> AurMutationPaths( IEnumerable<(string Channel, string Directory)> selections )
    {
        foreach ( var selection in selections )
        {
            yield return Path.Combine( selection.Directory, RepositoryNames.PackageBuildFile );
            yield return Path.Combine( selection.Directory, RepositoryNames.SourceInfoFile );
            if ( selection.Channel == PackageChannels.Source )
            {
                yield return Path.Combine( selection.Directory, "wayscriber.install" );
            }
            if ( selection.Channel == PackageChannels.Binary )
            {
                yield return Path.Combine( selection.Directory, "wayscriber-bin.install" );
            }
        }
    }

    private static string ChecksumFor( string channel, string sourceSha, string binarySha ) => channel == PackageChannels.Binary ? binarySha : sourceSha;

    private static async Task PublishAurUpdates( ToolContext context, IEnumerable<(string Channel, string Directory)> selections, string version )
    {
        var gitEnvironment = AurGitEnvironment( context );
        foreach ( var selection in selections )
        {
            var paths = new List<string> { RepositoryNames.PackageBuildFile, RepositoryNames.SourceInfoFile };
            var obsolete = selection.Channel == PackageChannels.Source ? "wayscriber.install" : selection.Channel == PackageChannels.Binary ? "wayscriber-bin.install" : null;
            await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, selection.Directory, "add", RepositoryNames.PackageBuildFile, RepositoryNames.SourceInfoFile] );
            if ( obsolete is not null && await RemoveTrackedFile( context, selection.Directory, obsolete ) )
            {
                paths.Add( obsolete );
            }

            var status = await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, selection.Directory, "status", "--porcelain", CommandLineOptions.EndOfOptions, .. paths], capture: true );
            if ( status.StandardOutput.Length > 0 )
            {
                await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, selection.Directory, "commit", "-m", $"{PackageName( selection.Channel )} {version}", CommandLineOptions.EndOfOptions, .. paths] );
            }
        }

        foreach ( var selection in selections )
        {
            await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, selection.Directory, "push"], environment: gitEnvironment );
        }
    }

    private static async Task<bool> RemoveTrackedFile( ToolContext context, string directory, string path )
    {
        var tracked = await context.Run(
            Programs.Git,
            [CommandLineOptions.ChangeDirectory, directory, "ls-files", "--error-unmatch", path],
            capture: true,
            allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( tracked.ExitCode != ExitCodes.Success )
        {
            return false;
        }

        await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "rm", "-f", "--ignore-unmatch", path] );
        return true;
    }

    private static async Task<int> UpdateAurInteractive( ToolContext context, string[] args )
    {
        if ( Console.IsInputRedirected )
        {
            throw new ToolException( "aur update --interactive requires a terminal." );
        }
        var parsed = new Arguments( args );
        var cargoVersion = VersionCommands.ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        var version = parsed.TakeOption( CommandLineOptions.Version ) ?? cargoVersion;
        var home = context.Environment( EnvironmentVariables.Home );
        if ( home is null )
        {
            throw new ToolException( ToolMessages.HomeNotSet );
        }

        var directory = Path.GetFullPath( parsed.TakeOption( "--source-dir" ) ?? Path.Combine( home, "aur-packages", RepositoryNames.MainPackage ) );
        parsed.RequireEmpty( "aur update --interactive [--version VERSION] [--source-dir DIR]" );
        var release = ReleaseVersion.Parse( version );
        if ( release.CargoVersion != cargoVersion )
        {
            throw new ToolException( $"Release version {version} is not a release or hotfix of Cargo version {cargoVersion}." );
        }

        var tag = "v" + version;
        var tagExists = await context.Run( Programs.Git, ["rev-parse", "-q", "--verify", $"refs/tags/{tag}"], capture: true,
            allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( tagExists.ExitCode != ExitCodes.Success )
        {
            if ( !Confirm( $"Create and push tag {tag}?" ) )
            {
                throw new ToolException( "Aborted: a release tag is required for AUR." );
            }
            await EnsureVersionAndCleanTree( context, version );
            await context.Run( Programs.Git, ["tag", "-a", tag, "-m", $"Release {tag}"] );
            try
            {
                await context.Run( Programs.Git, ["push", "origin", tag] );
            }
            catch { await context.Run( Programs.Git, ["tag", "-d", tag], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } ); throw; }
        }

        if ( !Directory.Exists( directory ) )
        {
            Directory.CreateDirectory( Path.GetDirectoryName( directory )! );
            await context.Run( Programs.Git, ["clone", $"ssh://aur@{AurHost}/{RepositoryNames.MainPackage}.git", directory], environment: AurGitEnvironment( context ) );
        }
        await ValidateAurCheckout( context, PackageChannels.Source, directory );
        var aurGitEnvironment = AurGitEnvironment( context );
        await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "fetch", "origin"], environment: aurGitEnvironment, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        var checkout = await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "checkout", "master"], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( checkout.ExitCode != ExitCodes.Success )
        {
            await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "checkout", "-b", "master"] );
        }
        await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "pull", "--rebase", "origin", "master"], environment: aurGitEnvironment, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );

        var sourceChecksum = await ResolveSourceChecksum( context, version, null );
        var packageBuild = Files.Read( context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.PackageBuildFile ) );
        packageBuild = Replace( packageBuild, @"(?m)^pkgver=.*$", $"pkgver={version}", "pkgver" );
        packageBuild = Replace( packageBuild, @"(?m)^pkgrel=.*$", "pkgrel=1", "pkgrel" );
        packageBuild = ReplaceArray( packageBuild, "sha256sums", $"sha256sums=('{sourceChecksum}')" );
        Files.WriteAtomic( Path.Combine( directory, RepositoryNames.PackageBuildFile ), packageBuild );
        File.Delete( Path.Combine( directory, "wayscriber.install" ) );
        var sourceInfo = await context.Run( Programs.Makepkg, ["--printsrcinfo"], directory, capture: true );
        Files.WriteAtomic( Path.Combine( directory, RepositoryNames.SourceInfoFile ), sourceInfo.StandardOutput );

        if ( Confirm( "Test build locally?" ) )
        {
            await context.Run( Programs.Makepkg, ["-f"], directory );
            if ( Confirm( "Install locally to test?" ) )
            {
                await context.Run( Programs.Makepkg, ["-i"], directory );
            }
        }
        await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "status", "--short"] );
        if ( !Confirm( "Push to AUR?" ) )
        {
            await context.Output.WriteLineAsync( $"AUR changes remain in {directory}." );
            return ExitCodes.Success;
        }
        await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "add", RepositoryNames.PackageBuildFile, RepositoryNames.SourceInfoFile] );
        var obsoleteRemoved = await RemoveTrackedFile( context, directory, "wayscriber.install" );
        var commitPaths = obsoleteRemoved
            ? new[] { RepositoryNames.PackageBuildFile, RepositoryNames.SourceInfoFile, "wayscriber.install" }
            : [RepositoryNames.PackageBuildFile, RepositoryNames.SourceInfoFile];
        await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "commit", "-m", $"Update to {tag}", CommandLineOptions.EndOfOptions, .. commitPaths] );
        var push = await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "push", "origin", "master"], environment: aurGitEnvironment, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( push.ExitCode != ExitCodes.Success )
        {
            await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "push", "-u", "origin", "master"], environment: aurGitEnvironment );
        }
        await context.Output.WriteLineAsync( $"Published {RepositoryNames.MainPackage} {version} to AUR." );
        return ExitCodes.Success;
    }

    private static bool Confirm( string prompt )
    {
        Console.Write( $"{prompt} [y/N] " );
        var response = Console.ReadLine( );
        return response is not null && (response.Equals( "y", StringComparison.OrdinalIgnoreCase ) || response.Equals( "yes", StringComparison.OrdinalIgnoreCase ));
    }

    private static async Task ValidateAurCheckout( ToolContext context, string channel, string directory )
    {
        if ( !File.Exists( Path.Combine( directory, RepositoryNames.PackageBuildFile ) ) || !File.Exists( Path.Combine( directory, RepositoryNames.SourceInfoFile ) ) )
        {
            throw new ToolException( $"{channel} AUR clone is missing {RepositoryNames.PackageBuildFile} or {RepositoryNames.SourceInfoFile}: {directory}" );
        }
        var root = await context.Run( Programs.Git, [CommandLineOptions.ChangeDirectory, directory, "rev-parse", "--show-toplevel"], capture: true );
        if ( Path.GetFullPath( root.StandardOutput.Trim( ) ) != Path.GetFullPath( directory ) )
        {
            throw new ToolException( $"{channel} AUR path is not the root of its Git worktree: {directory}" );
        }
    }

    private static string ArtifactSha( JsonElement manifest, string name )
    {
        var matches = manifest.GetProperty( "artifacts" ).EnumerateArray( ).Where( item => item.GetProperty( "name" ).GetString( ) == name ).ToArray( );
        if ( matches.Length != 1 )
        {
            throw new ToolException( $"Expected exactly one checksum for artifact {name}, found {matches.Length}" );
        }
        var value = matches[0].GetProperty( "sha256" ).GetString( ) ?? string.Empty;
        if ( !Regex.IsMatch( value, HashingConstants.Sha256HexPattern ) )
        {
            throw new ToolException( $"Artifact checksum for {name} is invalid." );
        }
        return value.ToLowerInvariant( );
    }

    private static async Task TransformAur( ToolContext context, string channel, string directory, string version, string sourceSha, string binarySha, JsonObject recipes )
    {
        var path = Path.Combine( directory, RepositoryNames.PackageBuildFile );
        var text = Files.Read( path );
        var currentVersion = Regex.Match( text, @"(?m)^pkgver=(.*)$" ).Groups[1].Value.Trim( );
        var currentReleaseText = Regex.Match( text, @"(?m)^pkgrel=(\d+)$" ).Groups[1].Value;
        var nextRelease = currentVersion == version && int.TryParse( currentReleaseText, out var currentRelease ) ? currentRelease + 1 : 1;
        text = Replace( text, @"(?m)^pkgver=.*$", $"pkgver={version}", "pkgver" );
        text = Replace( text, @"(?m)^pkgrel=.*$", $"pkgrel={nextRelease}", "pkgrel" );
        text = Regex.Replace( text, @"(?m)^install=.*\n?", string.Empty );
        text = Regex.Replace( text, @"(?m)^\s*'git'\s*\n", string.Empty );
        text = EnsureDependency( text, "libxkbcommon", "gcc-libs" );
        if ( channel == PackageChannels.Binary )
        {
            text = EnsureDependency( text, "gtk4", "wl-clipboard" );
            text = RemoveDependency( text, RepositoryNames.Gtk4LayerShell );
            text = ReplaceArray( text, "source_x86_64", $"source_x86_64=(\"wayscriber-v{version}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v{version}/wayscriber-v{version}-linux-x86_64.tar.gz\")" );
            text = ReplaceArray( text, "sha256sums_x86_64", $"sha256sums_x86_64=('{binarySha}')" );
            if ( !text.Contains( "usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell", StringComparison.Ordinal ) )
            {
                var license = "    install -Dm644 \"${srcdir_tmp}/usr/share/licenses/wayscriber/LICENSE.gtk4-layer-shell\" \"$pkgdir/usr/share/licenses/$pkgname/LICENSE.gtk4-layer-shell\"";
                text = new Regex( @"(?m)^(.*usr/share/doc/wayscriber/LICENSE.*)$" ).Replace( text, $"$1\n{license}", 1 );
            }
        }
        else
        {
            text = EnsureDependency( text, "gtk4", channel == PackageChannels.Configurator ? "gcc-libs" : "wl-clipboard" );
            if ( channel == PackageChannels.Source )
            {
                text = EnsureDependency( text, RepositoryNames.Gtk4LayerShell, "wl-clipboard" );
            }

            if ( channel == PackageChannels.Configurator )
            {
                text = EnsureDependency( text, $"libadwaita>={VersionCommands.SupportedLibadwaitaFloor}", "gcc-libs" );
                text = Replace( text, @"(?m)^pkgdesc=.*$", "pkgdesc='GUI configurator for wayscriber (GTK4/libadwaita)'", "pkgdesc" );
            }
            text = ReplaceArray( text, "source", "source=(\"wayscriber-$pkgver.tar.gz::https://github.com/devmobasa/wayscriber/archive/refs/tags/v$pkgver.tar.gz\")" );
            text = ReplaceArray( text, "sha256sums", $"sha256sums=('{sourceSha}')" );
            text = Regex.Replace( text, """(?m)^\s*cd (?:"\$pkgname"|wayscriber)$""", "    cd \"wayscriber-$pkgver\"" );
            text = Regex.Replace( text, "\"\\$srcdir/(wayscriber(?:-configurator)?\\.desktop)\"", "packaging/$1" );
            text = Regex.Replace( text, "\"\\$srcdir/(wayscriber(?:-configurator)?-[0-9]+\\.png)\"", "packaging/icons/$1" );
        }
        text = ApplyDesktopAssets( text, recipes[channel]!.AsObject( ) );
        Files.WriteAtomic( path, text );
        var srcinfoPath = Path.Combine( directory, RepositoryNames.SourceInfoFile );
        var srcinfo = TransformSrcInfo( Files.Read( srcinfoPath ), channel, version, nextRelease, sourceSha, binarySha );
        Files.WriteAtomic( srcinfoPath, srcinfo );
        if ( channel == PackageChannels.Source )
        {
            File.Delete( Path.Combine( directory, "wayscriber.install" ) );
        }
        if ( channel == PackageChannels.Binary )
        {
            File.Delete( Path.Combine( directory, "wayscriber-bin.install" ) );
        }
    }

    private static string TransformSrcInfo( string text, string channel, string version, int release, string sourceSha, string binarySha )
    {
        text = SetSrcInfoField( text, "pkgver", version );
        text = SetSrcInfoField( text, "pkgrel", release.ToString( System.Globalization.CultureInfo.InvariantCulture ) );
        text = RemoveSrcInfoField( text, "install" );
        text = RemoveSrcInfoValue( text, "makedepends", Programs.Git );
        text = EnsureSrcInfoDependency( text, "libxkbcommon", "gcc-libs" );
        if ( channel == PackageChannels.Binary )
        {
            text = EnsureSrcInfoDependency( text, "gtk4", "wl-clipboard" );
            text = RemoveSrcInfoValue( text, "depends", RepositoryNames.Gtk4LayerShell );
            text = SetSrcInfoField( text, "source_x86_64", $"wayscriber-v{version}-linux-x86_64.tar.gz::https://github.com/devmobasa/wayscriber/releases/download/v{version}/wayscriber-v{version}-linux-x86_64.tar.gz" );
            text = SetSrcInfoField( text, "sha256sums_x86_64", binarySha );
        }
        else
        {
            text = EnsureSrcInfoDependency( text, "gtk4", channel == PackageChannels.Configurator ? "gcc-libs" : "wl-clipboard" );
            if ( channel == PackageChannels.Source )
            {
                text = EnsureSrcInfoDependency( text, RepositoryNames.Gtk4LayerShell, "wl-clipboard" );
            }
            if ( channel == PackageChannels.Configurator )
            {
                text = EnsureSrcInfoDependency( text, $"libadwaita>={VersionCommands.SupportedLibadwaitaFloor}", "gcc-libs" );
                text = SetSrcInfoField( text, "pkgdesc", "GUI configurator for wayscriber (GTK4/libadwaita)" );
            }
            text = SetSrcInfoField( text, "source", $"wayscriber-{version}.tar.gz::https://github.com/devmobasa/wayscriber/archive/refs/tags/v{version}.tar.gz" );
            text = SetSrcInfoField( text, "sha256sums", sourceSha );
        }
        return text;
    }

    private static string SetSrcInfoField( string text, string field, string value )
    {
        var lines = text.Split( '\n' ).ToList( );
        var found = false;
        for ( var index = 0; index < lines.Count; index++ )
        {
            if ( !Regex.IsMatch( lines[index], $@"^\s*{Regex.Escape( field )} = " ) )
            {
                continue;
            }
            if ( !found )
            {
                lines[index] = $"\t{field} = {value}";
                found = true;
            }
            else
            {
                lines.RemoveAt( index-- );
            }
        }
        if ( !found )
        {
            var packageIndex = lines.FindIndex( line => Regex.IsMatch( line, @"^\s*pkgname = " ) );
            lines.Insert( packageIndex >= 0 ? packageIndex : lines.Count, $"\t{field} = {value}" );
        }
        return string.Join( '\n', lines );
    }

    private static string RemoveSrcInfoField( string text, string field ) =>
        string.Join( '\n', text.Split( '\n' ).Where( line => !Regex.IsMatch( line, $@"^\s*{Regex.Escape( field )} = " ) ) );

    private static string RemoveSrcInfoValue( string text, string field, string value ) =>
        string.Join( '\n', text.Split( '\n' ).Where( line => !Regex.IsMatch( line, $@"^\s*{Regex.Escape( field )} = {Regex.Escape( value )}\s*$" ) ) );

    private static string EnsureSrcInfoDependency( string text, string dependency, string anchor )
    {
        if ( Regex.IsMatch( text, $@"(?m)^\s*depends = {Regex.Escape( dependency )}\s*$" ) )
        {
            return text;
        }
        var pattern = $@"(?m)^(\s*)depends = {Regex.Escape( anchor )}\s*$";
        if ( !Regex.IsMatch( text, pattern ) )
        {
            throw new ToolException( $".SRCINFO lacks dependency anchor '{anchor}' needed to add '{dependency}'." );
        }
        return new Regex( pattern ).Replace( text, $"$1depends = {dependency}\n$0", 1 );
    }

    internal static string ApplyDesktopAssets( string text, JsonObject recipe )
    {
        var marker = recipe["marker"]!.GetValue<string>( );
        var end = recipe["end_marker"]!.GetValue<string>( );
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var block = DesktopAssetBlock( recipe );
        var markerCount = text.Split( '\n' ).Count( line => line == marker );
        var endCount = text.Split( '\n' ).Count( line => line == end );
        if ( markerCount > 1 || endCount > 1 || markerCount == 0 && endCount != 0 )
        {
            throw new ToolException( $"Malformed desktop asset block: {marker}" );
        }

        if ( markerCount == 1 && endCount == 1 )
        {
            var match = Regex.Match( text, DesktopAssetBlockPattern( marker, end ) );
            if ( !match.Success )
            {
                throw new ToolException( $"Malformed desktop asset block: {marker}" );
            }

            return match.Value == block
                ? text
                : text[..match.Index] + block + text[(match.Index + match.Length)..];
        }

        if ( markerCount == 1 )
        {
            text = RemoveIncompleteDesktopAssetBlock( text, marker, end, recipe );
        }
        else if ( HasMarkerlessDesktopAssets( text, recipe ) )
        {
            text = RemoveMarkerlessDesktopAssets( text, recipe );
        }

        var destinations = recipe["destinations"]!.AsArray( ).Select( item => item!.GetValue<string>( ) ).ToArray( );
        if ( destinations.Any( destination => text.Contains( destination, StringComparison.Ordinal ) ) )
        {
            throw new ToolException( $"{recipe["label"]!.GetValue<string>( )} PKGBUILD has unmanaged desktop integration." );
        }
        if ( !text.Contains( anchor, StringComparison.Ordinal ) )
        {
            throw new ToolException( $"PKGBUILD has no binary install line to extend: {anchor}" );
        }

        return text.Replace( anchor, anchor + "\n\n" + block, StringComparison.Ordinal );
    }

    private static string RemoveIncompleteDesktopAssetBlock( string text, string marker, string end, JsonObject recipe )
    {
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var isConfigurator = anchor.Contains( RepositoryNames.ConfiguratorPackage, StringComparison.Ordinal );
        var lines = text.Split( '\n' ).Where( line => line != marker && line != end &&
            !IsDesktopAssetInstallForPackage( line, isConfigurator ) );
        return string.Join( '\n', lines );
    }

    private static string RemoveMarkerlessDesktopAssets( string text, JsonObject recipe )
    {
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var isConfigurator = anchor.Contains( RepositoryNames.ConfiguratorPackage, StringComparison.Ordinal );
        var lines = text.Split( '\n' ).Where( line => !IsDesktopAssetInstallForPackage( line, isConfigurator ) );
        return string.Join( '\n', lines );
    }

    private static bool HasMarkerlessDesktopAssets( string text, JsonObject recipe )
    {
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var isConfigurator = anchor.Contains( RepositoryNames.ConfiguratorPackage, StringComparison.Ordinal );
        return text.Split( '\n' ).Any( line => IsDesktopAssetInstallForPackage( line, isConfigurator ) );
    }

    private static bool IsDesktopAssetInstallForPackage( string line, bool isConfigurator )
    {
        if ( !Regex.IsMatch( line, @"^\s*install\s+.*\$pkgdir/usr/share/(?:applications|icons|pixmaps)/" ) )
        {
            return false;
        }

        var containsConfigurator = line.Contains( CommandNames.Configurator, StringComparison.Ordinal );
        return isConfigurator || !containsConfigurator;
    }

    private static void ValidateAurRecipe( string channel, string directory, string version, string sha, JsonObject recipes )
    {
        var pkgbuild = Files.Read( Path.Combine( directory, RepositoryNames.PackageBuildFile ) );
        var srcinfo = Files.Read( Path.Combine( directory, RepositoryNames.SourceInfoFile ) );
        var packageRelease = Regex.Match( pkgbuild, @"(?m)^pkgrel=([1-9][0-9]*)$" ).Groups[1].Value;
        if ( !Regex.IsMatch( pkgbuild, $@"(?m)^pkgver={Regex.Escape( version )}$" ) || !Regex.IsMatch( pkgbuild, $@"'{Regex.Escape( sha )}'" ) ||
            packageRelease.Length == 0 || !Regex.IsMatch( srcinfo, $@"(?m)^\s*pkgver = {Regex.Escape( version )}$" ) ||
            !Regex.IsMatch( srcinfo, $@"(?m)^\s*pkgrel = {Regex.Escape( packageRelease )}$" ) || !srcinfo.Contains( sha, StringComparison.Ordinal ) )
        {
            throw new ToolException( $"{channel} PKGBUILD and .SRCINFO metadata disagree." );
        }
        IReadOnlyList<string>? dependencies = channel switch
        {
            PackageChannels.Source => new[] { "libxkbcommon", "gtk4", RepositoryNames.Gtk4LayerShell },
            PackageChannels.Binary => new[] { "libxkbcommon", "gtk4" },
            PackageChannels.Configurator => new[] { "libxkbcommon", "gtk4", $"libadwaita>={VersionCommands.SupportedLibadwaitaFloor}" },
            _ => null,
        };
        if ( dependencies is null )
        {
            throw new ToolException( $"Unknown AUR channel: {channel}" );
        }

        foreach ( var dependency in dependencies )
        {
            if ( !Regex.IsMatch( pkgbuild, $@"(?m)^\s*'{Regex.Escape( dependency )}'\s*$" ) ||
                !Regex.IsMatch( srcinfo, $@"(?m)^\s*depends = {Regex.Escape( dependency )}\s*$" ) )
            {
                throw new ToolException( $"{channel} recipe lacks dependency {dependency}." );
            }
        }
        if ( channel == PackageChannels.Binary && (pkgbuild.Contains( "'gtk4-layer-shell'", StringComparison.Ordinal ) || Regex.IsMatch( srcinfo, @"(?m)^\s*depends = gtk4-layer-shell\s*$" )) )
        {
            throw new ToolException( "wayscriber-bin retains a dynamic gtk4-layer-shell dependency." );
        }
        if ( Regex.IsMatch( srcinfo, @"(?m)^\s*(?:install|makedepends) = (?:wayscriber(?:-bin)?\.install|git)\s*$" ) )
        {
            throw new ToolException( $"{channel} .SRCINFO retains obsolete install or git metadata." );
        }
        var recipe = recipes[channel]!.AsObject( );
        if ( !HasExactDesktopAssetBlock( pkgbuild, recipe ) )
        {
            throw new ToolException( $"{channel} PKGBUILD has a missing, stale, or malformed desktop asset block." );
        }
    }

    private static string DesktopAssetBlock( JsonObject recipe ) =>
        recipe["marker"]!.GetValue<string>( ) + "\n" +
        string.Join( '\n', recipe["lines"]!.AsArray( ).Select( item => item!.GetValue<string>( ) ) ) + "\n" +
        recipe["end_marker"]!.GetValue<string>( );

    private static string DesktopAssetBlockPattern( string marker, string end ) =>
        $@"(?ms)^{Regex.Escape( marker )}\n.*?^{Regex.Escape( end )}$";

    internal static bool HasExactDesktopAssetBlock( string text, JsonObject recipe )
    {
        var marker = recipe["marker"]!.GetValue<string>( );
        var end = recipe["end_marker"]!.GetValue<string>( );
        var matches = Regex.Matches( text, DesktopAssetBlockPattern( marker, end ) );
        return matches.Count == 1 && matches[0].Value == DesktopAssetBlock( recipe );
    }

    private static string Replace( string text, string pattern, string value, string label )
    {
        if ( Regex.Matches( text, pattern ).Count != 1 )
        {
            throw new ToolException( $"Expected one {label} field." );
        }
        return Regex.Replace( text, pattern, value );
    }
    private static string ReplaceArray( string text, string name, string value ) => Replace( text, $@"(?ms)^{Regex.Escape( name )}=\([^)]*\)", value, name );
    private static string EnsureDependency( string text, string dependency, string anchor )
    {
        if ( Regex.IsMatch( text, $@"(?m)^\s*'{Regex.Escape( dependency )}'\s*$" ) )
        {
            return text;
        }

        var pattern = $@"(?m)^(\s*'{Regex.Escape( anchor )}'\s*)$";
        if ( !Regex.IsMatch( text, pattern ) )
        {
            throw new ToolException( $"PKGBUILD lacks dependency anchor '{anchor}' needed to add '{dependency}'." );
        }

        return new Regex( pattern ).Replace( text, $"    '{dependency}'\n$1", 1 );
    }
    private static string RemoveDependency( string text, string dependency ) => Regex.Replace( text, $@"(?m)^\s*'{Regex.Escape( dependency )}'\s*\n", string.Empty );
    private static string PackageName( string channel )
    {
        var packageName = channel switch
        {
            PackageChannels.Source => RepositoryNames.MainPackage,
            PackageChannels.Binary => RepositoryNames.BinaryPackage,
            PackageChannels.Configurator => RepositoryNames.ConfiguratorPackage,
            _ => null,
        };
        if ( packageName is null )
        {
            throw new ToolException( $"Unknown AUR channel: {channel}" );
        }

        return packageName;
    }

    private static void CopyTree( string source, string destination, bool includeGit )
    {
        Directory.CreateDirectory( destination );
        foreach ( var file in Directory.EnumerateFiles( source ) )
        {
            File.Copy( file, Path.Combine( destination, Path.GetFileName( file ) ), true );
        }

        foreach ( var directory in Directory.EnumerateDirectories( source ) )
        {
            if ( includeGit || Path.GetFileName( directory ) != ".git" )
            {
                CopyTree( directory, Path.Combine( destination, Path.GetFileName( directory ) ), includeGit );
            }
        }
    }

    private static async Task<int> PrepareSsh( ToolContext context, string[] args )
    {
        if ( OperatingSystem.IsWindows( ) )
        {
            throw new ToolException( "AUR SSH setup requires Unix." );
        }
        new Arguments( args ).RequireEmpty( "aur prepare-ssh" );
        var key = context.Environment( EnvironmentVariables.AurSshPrivateKey );
        if ( key is null )
        {
            throw new ToolException( "AUR_SSH_PRIVATE_KEY is missing." );
        }

        var home = context.Environment( EnvironmentVariables.Home );
        if ( home is null )
        {
            throw new ToolException( "HOME is missing." );
        }

        var directory = Path.Combine( home, ".ssh" );
        Directory.CreateDirectory( directory );
        var privateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
        var keyPath = Path.Combine( directory, AurKeyFileName );
        Files.WriteAtomic( keyPath, EnsureTrailingNewline( key ), privateMode );

        var knownHostsPath = Path.Combine( directory, AurKnownHostsFileName );
        var knownHosts = context.Environment( EnvironmentVariables.AurSshKnownHosts );
        if ( knownHosts is null )
        {
            var scan = await context.Run( Programs.SshKeyScan, ["-H", AurHost], capture: true );
            knownHosts = scan.StandardOutput;
        }

        Files.WriteAtomic( knownHostsPath, knownHosts, privateMode );

        var githubEnvironment = context.Environment( EnvironmentVariables.GitHubEnvironment );
        var processEnvironment = new Dictionary<string, string?>( AurGitEnvironment( context ) );
        if ( context.Environment( EnvironmentVariables.AurSshPassphrase ) is { Length: > 0 } passphrase )
        {
            var agent = await context.Run( Programs.SshAgent, ["-s"], capture: true );
            foreach ( var name in new[] { EnvironmentVariables.SshAuthSocket, EnvironmentVariables.SshAgentProcessId } )
            {
                var match = Regex.Match( agent.StandardOutput, $@"(?:^|\n){name}=([^;\n]+);" );
                if ( !match.Success )
                {
                    throw new ToolException( $"ssh-agent did not report {name}." );
                }
                processEnvironment[name] = match.Groups[1].Value;
            }

            var askpassEnvironment = new Dictionary<string, string?>( processEnvironment )
            {
                [EnvironmentVariables.SshAskpass] = context.Path( RepositoryPaths.ToolsDirectory, "wayscriber.cs" ),
                [EnvironmentVariables.SshAskpassRequire] = EnvironmentVariables.Force,
                [EnvironmentVariables.Display] = ":0",
                [EnvironmentVariables.WayscriberSshAskpass] = EnvironmentVariables.Enabled,
                [EnvironmentVariables.AurSshPassphrase] = passphrase,
            };
            await context.Run( Programs.Setsid, ["-w", Programs.SshAdd, keyPath], environment: askpassEnvironment, input: string.Empty, capture: true );
        }

        if ( githubEnvironment is not null )
        {
            var values = processEnvironment.Where( pair => pair.Key is EnvironmentVariables.SshAuthSocket or EnvironmentVariables.SshAgentProcessId )
                .Select( pair => $"{pair.Key}={pair.Value}\n" );
            await File.AppendAllTextAsync( githubEnvironment, string.Concat( values ) + $"GIT_SSH_COMMAND={processEnvironment[EnvironmentVariables.GitSshCommand]}\n", context.CancellationToken );
        }

        await context.Run( Programs.Git, ["ls-remote", $"ssh://aur@{AurHost}/wayscriber.git", "HEAD"], environment: processEnvironment );
        await context.Output.WriteLineAsync( $"AUR SSH key prepared at {keyPath}" );
        return ExitCodes.Success;
    }

    private static async Task<int> ConfigureGit( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "aur configure-git" );
        var name = context.Environment( EnvironmentVariables.AurGitUserName );
        if ( name is null )
        {
            throw new ToolException( "AUR_GIT_USERNAME is missing." );
        }

        var email = context.Environment( EnvironmentVariables.AurGitEmail );
        if ( email is null )
        {
            throw new ToolException( "AUR_GIT_EMAIL is missing." );
        }

        await context.Run( Programs.Git, ["config", "--global", "user.name", name] );
        await context.Run( Programs.Git, ["config", "--global", "user.email", email] );
        return ExitCodes.Success;
    }

    private static async Task<int> CloneAur( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var root = Path.GetFullPath( parsed.TakeOption( "--directory" ) ?? context.RepositoryRoot, Environment.CurrentDirectory );
        parsed.RequireEmpty( "aur clone [--directory PATH]" );
        var gitEnvironment = AurGitEnvironment( context );
        foreach ( var (repository, directory) in new[]
        {
            (RepositoryNames.MainPackage, "aur-wayscriber"),
            ("wayscriber-bin", "aur-wayscriber-bin"),
            (RepositoryNames.ConfiguratorPackage, "aur-wayscriber-configurator"),
        } )
        {
            await context.Run( Programs.Git, ["clone", $"ssh://aur@{AurHost}/{repository}.git", Path.Combine( root, directory )], environment: gitEnvironment );
        }
        return ExitCodes.Success;
    }

    private static IReadOnlyDictionary<string, string?> AurGitEnvironment( ToolContext context )
    {
        var home = context.Environment( EnvironmentVariables.Home );
        if ( home is null )
        {
            return new Dictionary<string, string?>( );
        }

        var directory = Path.Combine( home, ".ssh" );
        var keyPath = Path.Combine( directory, AurKeyFileName );
        var knownHostsPath = Path.Combine( directory, AurKnownHostsFileName );
        if ( !File.Exists( keyPath ) || !File.Exists( knownHostsPath ) )
        {
            return new Dictionary<string, string?>( );
        }

        return new Dictionary<string, string?>
        {
            [EnvironmentVariables.GitSshCommand] = ProcessRunner.FormatCommand(
                Programs.Ssh,
                ["-i", keyPath, "-o", "IdentitiesOnly=yes", "-o", "StrictHostKeyChecking=yes", "-o", $"UserKnownHostsFile={knownHostsPath}"] ),
        };
    }

    private static async Task<int> DeployRepositories( ToolContext context, string[] args )
    {
        if ( OperatingSystem.IsWindows( ) )
        {
            throw new ToolException( "Repository deployment requires Unix." );
        }
        new Arguments( args ).RequireEmpty( "release deploy-package-repositories" );
        var host = context.Environment( EnvironmentVariables.DeployHost );
        if ( host is null )
        {
            throw new ToolException( "DEPLOY_HOST is missing." );
        }

        var path = context.Environment( EnvironmentVariables.DeployPath );
        if ( path is null )
        {
            throw new ToolException( "DEPLOY_PATH is missing." );
        }

        var user = context.Environment( EnvironmentVariables.DeployUser ) ?? "root";
        var key = context.Environment( EnvironmentVariables.PackageRepositorySshKey );
        if ( key is null )
        {
            throw new ToolException( "PACKAGE_REPO_SSH_KEY is missing." );
        }

        using var temporary = new TemporaryDirectory( "wayscriber-deploy" );
        var keyPath = Path.Combine( temporary.Path, "key" );
        Files.WriteAtomic( keyPath, EnsureTrailingNewline( key ), UnixFileMode.UserRead | UnixFileMode.UserWrite );
        var knownHostsPath = Path.Combine( temporary.Path, "known_hosts" );
        var knownHosts = context.Environment( EnvironmentVariables.PackageRepositorySshKnownHosts );
        if ( string.IsNullOrWhiteSpace( knownHosts ) )
        {
            var scan = await context.Run( Programs.SshKeyScan, ["-H", host], capture: true );
            knownHosts = scan.StandardOutput;
        }
        Files.WriteAtomic( knownHostsPath, knownHosts );
        var target = $"{user}@{host}";
        await context.Run( Programs.Ssh, ["-i", keyPath, "-o", "StrictHostKeyChecking=yes", "-o", $"UserKnownHostsFile={knownHostsPath}", target, "mkdir", "-p", $"{path}/apt", $"{path}/rpm"] );
        foreach ( var kind in new[] { PackageFormats.Apt, PackageFormats.Rpm } )
        {
            await context.Run( Programs.Rsync, ["-rlvz", "--omit-dir-times", "--delete", "--no-perms", "--no-owner", "--no-group", "-e", $"ssh -i {keyPath} -o StrictHostKeyChecking=yes -o UserKnownHostsFile={knownHostsPath}", $"repo-out/{kind}/", $"{target}:{path}/{kind}/"] );
        }

        return ExitCodes.Success;
    }

    private static string EnsureTrailingNewline( string value ) =>
        value.EndsWith( '\n' ) ? value : value + "\n";
}
