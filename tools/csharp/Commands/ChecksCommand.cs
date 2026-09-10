using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal static class ChecksCommand
{
    private const string LibraryFilePrefix = "lib";
    private static readonly string[] StandaloneToolFiles =
    [
        "build-package-repos.sh", "build.sh", "bump-version.sh", "check-arch-installer-manifest.sh",
        "check-config-writers.py", "check-nixpkgs-recipe.py", "check-process-sites.py",
        "check-rust-source-coverage.py", "check-shared-dependencies.py", "check-version-consistency.sh",
        "code-health-report.sh", "create-release-tag.sh", "fetch-all-deps.sh", "install-configurator.sh",
        "install-gtk4-layer-shell.sh", "install.sh", "lint-and-test.sh", "package.sh",
        "publish-release-tag.sh", "reload-daemon.sh", "run.sh", "set-portal-shortcut.sh", "test-aur-desktop-assets.sh", "test-gtk-widgets.sh",
        "test-package-repo-layout.sh", "test-release-packaging.sh", "test.sh", "update-aur-from-manifest.sh",
        "update-aur.sh", "verify-static-gtk4-layer-shell.sh", "aur-desktop-assets.sh", "aur-desktop-assets.py",
    ];

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Check, CommandNames.SharedDependencies, "Guard shared-layer Rust dependencies.", SideEffect.ReadOnly, SharedDependencies ),
        new( CommandAreas.Check, CommandNames.ProcessSites, "Audit Rust process-creation ownership.", SideEffect.ReadOnly, ProcessSites ),
        new( CommandAreas.Check, CommandNames.RustSourceCoverage, "Verify every Rust source is compiled.", SideEffect.ReadOnly, RustSourceCoverage ),
        new( CommandAreas.Check, CommandNames.NixpkgsRecipe, "Check Cargo native dependencies against Nix.", SideEffect.ReadOnly, NixpkgsRecipe ),
        new( CommandAreas.Check, CommandNames.ConfigWriters, "Audit config write-capability ownership.", SideEffect.ReadOnly, ConfigWriters ),
        new( CommandAreas.Check, CommandNames.LegacyTools, "Verify standalone scripts remain independent fallbacks.", SideEffect.ReadOnly, LegacyTools ),
    ];

    private static Task<int> SharedDependencies( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check shared-dependencies" );
        var errors = new List<string>( );
        foreach ( var (directory, forbidden) in new[]
        {
            ("src/domain", "config|input|draw|backend|ui|session"),
            ("src/config/validate", "input|backend"),
        } )
        {
            foreach ( var path in Directory.EnumerateFiles( context.Path( directory.Split( '/' ) ), "*.rs", SearchOption.AllDirectories ) )
            {
                if ( Path.GetRelativePath( context.RepositoryRoot, path ) == "src/domain/tests.rs" )
                {
                    continue;
                }
                var source = Regex.Replace( Files.Read( path ), @"/\*.*?\*/|//[^\n]*", string.Empty, RegexOptions.Singleline );
                var pattern = $@"crate\s*::\s*(?:{forbidden})\b|use\s+crate\s*::\s*\{{[^;]*\b(?:{forbidden})\s*::";
                if ( Regex.IsMatch( source, pattern ) )
                {
                    errors.Add( $"{Path.GetRelativePath( context.RepositoryRoot, path )}: upward dependency in shared layer" );
                }
            }
        }
        Failures( context, errors, "Shared domain and configuration-validation dependency paths passed." );
        return Task.FromResult( ExitCodes.Success );
    }

    private static Task<int> ProcessSites( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check process-sites" );
        var errors = new List<string>( );
        var allow = new HashSet<string>( StringComparer.Ordinal )
        {
            "configurator/src/app/session_catalog.rs",
            "configurator/src/app/daemon_setup/command.rs",
            "configurator/src/app/daemon_setup/service.rs",
        };
        Regex[] patterns =
        [
            new( @"\b(?:std::process::)?Command::new\b" ),
            new( @"\bstd::process::Child\b" ),
            new( @"\blibc::(?:fork|vfork|posix_spawn|posix_spawnp|pthread_atfork)\b" ),
            new( @"\blibc::SYS_(?:clone|clone3|fork|vfork)\b" ),
            new( @"\b(?:sh|bash|zsh)\s+-c\b" ),
        ];
        foreach ( var root in new[] { "src", "configurator/src", "tests" } )
        {
            foreach ( var path in Directory.EnumerateFiles( context.Path( root.Split( '/' ) ), "*.rs", SearchOption.AllDirectories ) )
            {
                var relative = Path.GetRelativePath( context.RepositoryRoot, path ).Replace( Path.DirectorySeparatorChar, '/' );
                var parts = relative.Split( '/' );
                var allowed = relative.StartsWith( "src/process_broker/", StringComparison.Ordinal ) || allow.Contains( relative ) ||
                    parts.Contains( "tests" ) || Path.GetFileName( relative ) == "tests.rs";
                var lines = File.ReadAllLines( path );
                for ( var index = 0; index < lines.Length; index++ )
                {
                    var code = lines[index].Split( "//", 2 )[0];
                    if ( !allowed && patterns.Any( pattern => pattern.IsMatch( code ) ) )
                    {
                        errors.Add( $"{relative}:{index + 1}: unclassified process site: {lines[index].Trim( )}" );
                    }
                }
            }
        }

        var bootstrap = context.Path( "src", "process_broker", "bootstrap.rs" );
        var source = Files.Read( bootstrap );
        const string start = "    if pid == 0 {";
        const string end = "    drop(child_socket);";
        if ( !source.Contains( start, StringComparison.Ordinal ) || !source.Contains( end, StringComparison.Ordinal ) )
        {
            errors.Add( "src/process_broker/bootstrap.rs: raw-clone child-stub markers changed" );
        }
        else
        {
            var stub = source.Split( start, 2, StringSplitOptions.None )[1].Split( end, 2, StringSplitOptions.None )[0];
            foreach ( var token in new[] { "format!(", "log::", "panic!(", ".unwrap(", ".expect(", "drop(", "Command::", "CString::", "Vec::", "String::", "Box::" } )
            {
                if ( stub.Contains( token, StringComparison.Ordinal ) )
                {
                    errors.Add( $"src/process_broker/bootstrap.rs: child stub reaches banned token '{token}'" );
                }
            }
            var libcCalls = Regex.Matches( stub, @"libc::([A-Za-z0-9_]+)\s*\(" ).Select( match => match.Groups[1].Value ).ToHashSet( );
            foreach ( var unexpected in libcCalls.Except( ["syscall", "_exit"] ).Order( ) )
            {
                errors.Add( $"src/process_broker/bootstrap.rs: child stub reaches unapproved libc call: {unexpected}" );
            }
            var syscalls = Regex.Matches( stub, @"libc::SYS_([A-Za-z0-9_]+)" ).Select( match => match.Groups[1].Value ).ToHashSet( );
            foreach ( var unexpected in syscalls.Except( ["fcntl", "dup3", "setpgid", "exit_group", "close_range", "execve"] ).Order( ) )
            {
                errors.Add( $"src/process_broker/bootstrap.rs: child stub reaches unapproved syscall: {unexpected}" );
            }
        }
        Failures( context, errors, "process-site audit passed", "process-site audit failed:" );
        return Task.FromResult( ExitCodes.Success );
    }

    private static async Task<int> RustSourceCoverage( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check rust-source-coverage" );
        var metadataResult = await context.Run( Programs.Cargo, ["metadata", CommandLineOptions.Locked, "--no-deps", "--format-version", "1"], capture: true );
        using var metadata = JsonDocument.Parse( metadataResult.StandardOutput );
        var packageIds = metadata.RootElement.GetProperty( "packages" ).EnumerateArray( )
            .Select( package => package.GetProperty( "id" ).GetString( )! ).ToHashSet( StringComparer.Ordinal );
        if ( packageIds.Count == 0 )
        {
            throw new ToolException( "cargo metadata returned no workspace packages", ExitCodes.InvalidArguments );
        }

        var depInfos = await CollectRustDepInfos( context, packageIds );
        var covered = CollectCoveredRustSources( context, depInfos );
        var git = await context.Run( Programs.Git, ["ls-files", "-co", "--exclude-standard", CommandLineOptions.EndOfOptions, "*.rs"], capture: true );
        var repositorySources = git.StandardOutput.Split( '\n', StringSplitOptions.RemoveEmptyEntries )
            .Where( path => File.Exists( context.Path( path.Split( '/' ) ) ) ).ToHashSet( StringComparer.Ordinal );
        var missing = repositorySources.Except( covered ).Order( ).ToArray( );
        if ( missing.Length > 0 )
        {
            throw new ToolException( $"Rust source coverage check failed: {missing.Length} source file(s) are not compiled by the supported Cargo matrix:\n" +
                string.Join( '\n', missing.Select( path => $"- {path}" ) ) );
        }

        await context.Output.WriteLineAsync( $"Rust source coverage OK: {repositorySources.Count} source files covered across 2 Cargo configurations ({depInfos.Count} current dep-info files)." );
        return ExitCodes.Success;
    }

    private static async Task<HashSet<string>> CollectRustDepInfos( ToolContext context, HashSet<string> packageIds )
    {
        var depInfos = new HashSet<string>( StringComparer.Ordinal );
        foreach ( var (label, flag) in new[] { ("all features", CommandLineOptions.AllFeatures), ("no default features", CommandLineOptions.NoDefaultFeatures) } )
        {
            await context.Error.WriteLineAsync( $"Checking Rust source coverage ({label})..." );
            var result = await context.Run( Programs.Cargo, ["check", CommandLineOptions.Workspace, CommandLineOptions.Locked, CommandLineOptions.AllTargets, flag,
                "--message-format=json-render-diagnostics"], capture: true );
            var lineNumber = 0;
            foreach ( var line in result.StandardOutput.Split( '\n', StringSplitOptions.RemoveEmptyEntries ) )
            {
                lineNumber++;
                JsonDocument message;
                try
                {
                    message = JsonDocument.Parse( line );
                }
                catch ( JsonException error )
                {
                    throw new ToolException( $"invalid cargo JSON at line {lineNumber}: {error.Message}", ExitCodes.InvalidArguments );
                }
                using ( message )
                {
                    var root = message.RootElement;
                    if ( !root.TryGetProperty( "reason", out var reason ) || reason.GetString( ) != "compiler-artifact" ||
                        !root.TryGetProperty( "package_id", out var id ) || !packageIds.Contains( id.GetString( )! ) )
                    {
                        continue;
                    }
                    if ( root.GetProperty( "target" ).GetProperty( "kind" ).EnumerateArray( ).Any( value => value.GetString( ) == "custom-build" ) )
                    {
                        continue;
                    }
                    var found = false;
                    foreach ( var filenameValue in root.GetProperty( "filenames" ).EnumerateArray( ) )
                    {
                        var filename = filenameValue.GetString( )!;
                        var directory = Path.GetDirectoryName( filename )!;
                        var stem = Path.GetFileNameWithoutExtension( filename );
                        foreach ( var candidateStem in stem.StartsWith( LibraryFilePrefix, StringComparison.Ordinal )
                                     ? new[] { stem, stem[LibraryFilePrefix.Length..] }
                                     : [stem] )
                        {
                            var candidate = Path.Combine( directory, candidateStem + ".d" );
                            if ( File.Exists( candidate ) )
                            {
                                depInfos.Add( candidate );
                                found = true;
                            }
                        }
                    }
                    if ( !found )
                    {
                        throw new ToolException( $"cargo artifact for {root.GetProperty( "target" ).GetProperty( "name" ).GetString( )} has no adjacent dep-info file", ExitCodes.InvalidArguments );
                    }
                }
            }
        }

        return depInfos;
    }

    private static HashSet<string> CollectCoveredRustSources( ToolContext context, HashSet<string> depInfos )
    {
        var covered = new HashSet<string>( StringComparer.Ordinal ) { "build.rs" };
        foreach ( var depInfo in depInfos )
        {
            var firstRule = Files.Read( depInfo ).Replace( "\\\n", " ", StringComparison.Ordinal ).Split( "\n\n", 2 )[0];
            var separator = firstRule.IndexOf( ':' );
            if ( separator < 0 )
            {
                throw new ToolException( $"dep-info has no dependency rule: {depInfo}", ExitCodes.InvalidArguments );
            }
            foreach ( var token in ParseMakeWords( firstRule[(separator + 1)..] ).Where( token => token.EndsWith( ".rs", StringComparison.Ordinal ) ) )
            {
                var absolute = Path.IsPathRooted( token ) ? token : context.Path( token.Split( '/' ) );
                if ( File.Exists( absolute ) && Path.GetRelativePath( context.RepositoryRoot, absolute ) is var relative && !relative.StartsWith( ".." ) )
                {
                    covered.Add( relative.Replace( Path.DirectorySeparatorChar, '/' ) );
                }
            }
        }

        return covered;
    }

    private static IEnumerable<string> ParseMakeWords( string text )
    {
        var current = new StringBuilder( );
        var escaped = false;
        foreach ( var character in text )
        {
            if ( escaped )
            {
                current.Append( character );
                escaped = false;
            }
            else if ( character == '\\' )
            {
                escaped = true;
            }
            else if ( char.IsWhiteSpace( character ) )
            {
                if ( current.Length > 0 )
                {
                    yield return current.ToString( );
                    current.Clear( );
                }
            }
            else
            {
                current.Append( character );
            }
        }
        if ( current.Length > 0 )
        {
            yield return current.ToString( );
        }
    }

    private static async Task<int> NixpkgsRecipe( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check nixpkgs-recipe" );
        var result = await context.Run( Programs.Cargo, ["metadata", CommandLineOptions.Locked, "--format-version", "1", "--filter-platform", "x86_64-unknown-linux-gnu"], capture: true );
        using var metadata = JsonDocument.Parse( result.StandardOutput );
        var manifestPath = context.Path( RepositoryPaths.CargoManifest );
        var packages = metadata.RootElement.GetProperty( "packages" ).EnumerateArray( ).ToArray( );
        var manifest = packages.SingleOrDefault( package => package.GetProperty( "manifest_path" ).GetString( ) == manifestPath );
        if ( manifest.ValueKind == JsonValueKind.Undefined )
        {
            throw new ToolException( "cargo metadata did not contain Cargo.toml" );
        }
        var packageId = manifest.GetProperty( "id" ).GetString( );
        var node = metadata.RootElement.GetProperty( "resolve" ).GetProperty( "nodes" ).EnumerateArray( )
            .Single( entry => entry.GetProperty( "id" ).GetString( ) == packageId );
        var packageNames = packages.ToDictionary( package => package.GetProperty( "id" ).GetString( )!, package => package.GetProperty( "name" ).GetString( )! );
        var direct = manifest.GetProperty( "dependencies" ).EnumerateArray( )
            .Where( dependency => dependency.GetProperty( "kind" ).ValueKind == JsonValueKind.Null )
            .Select( dependency => dependency.GetProperty( "name" ).GetString( )! ).ToHashSet( );
        var enabled = node.GetProperty( "deps" ).EnumerateArray( )
            .Where( dependency => dependency.GetProperty( "dep_kinds" ).EnumerateArray( ).Any( kind => kind.GetProperty( "kind" ).ValueKind == JsonValueKind.Null ) )
            .Select( dependency => packageNames[dependency.GetProperty( "pkg" ).GetString( )!] ).ToHashSet( );
        var errors = new List<string>( );
        foreach ( var crate in direct.Except( SystemLibraries.Keys ).Order( ) )
        {
            errors.Add( $"dependency `{crate}` has no system-library declaration" );
        }
        var required = enabled.SelectMany( crate => SystemLibraries.GetValueOrDefault( crate, [] ).Select( attribute => (crate, attribute) ) )
            .GroupBy( pair => pair.attribute ).ToDictionary( group => group.Key, group => group.Select( pair => pair.crate ).ToArray( ) );
        var recipe = Files.Read( context.Path( RepositoryPaths.PackagingDirectory, "nixpkgs", "package.nix" ) );
        var flake = Files.Read( context.Path( "flake.nix" ) );
        var marker = flake.IndexOf( "wayscriber = rustPlatform.buildRustPackage", StringComparison.Ordinal );
        if ( marker < 0 )
        {
            errors.Add( "flake.nix: Wayscriber package marker is missing" );
            marker = 0;
        }
        var recipeNative = NixList( recipe, "nativeBuildInputs" );
        var recipeBuild = NixList( recipe, "buildInputs" );
        var flakeNative = NixList( flake, "nativeBuildInputs", marker );
        var flakeBuild = NixList( flake, "buildInputs", marker );
        var argumentsMatch = Regex.Match( recipe, @"\A\s*\{(.*?)\}\s*:", RegexOptions.Singleline );
        var recipeArguments = argumentsMatch.Success
            ? Regex.Matches( argumentsMatch.Groups[1].Value, @"[A-Za-z_][A-Za-z0-9_'-]*" ).Select( match => match.Value ).ToHashSet( )
            : [];
        foreach ( var attribute in new[] { "pkg-config", "wrapGAppsHook4" } )
        {
            RequireNixInput( errors, attribute, recipeNative, recipeArguments, flakeNative, native: true );
        }
        foreach ( var pair in required.OrderBy( pair => pair.Key ) )
        {
            RequireNixInput( errors, pair.Key, recipeBuild, recipeArguments, flakeBuild, native: false, string.Join( ", ", pair.Value.Order( ) ) );
        }
        Failures( context, errors,
            $"nixpkgs recipe OK: {enabled.Count} default-feature dependencies require {required.Count} system package(s) ({string.Join( ", ", required.Keys.Order( ) )}), plus 2 native input(s), all declared in packaging/nixpkgs/package.nix and flake.nix.",
            "nixpkgs recipe check failed:" );
        return ExitCodes.Success;
    }

    private static HashSet<string> NixList( string text, string attribute, int start = 0 )
    {
        var match = Regex.Match( text[start..], $@"(?<![A-Za-z]){Regex.Escape( attribute )}\s*=\s*(?:with\s+[\w.]+;\s*)?\[(.*?)\]", RegexOptions.Singleline );
        if ( !match.Success )
        {
            return [];
        }
        var body = Regex.Replace( match.Groups[1].Value, @"#[^\n]*", string.Empty );
        return Regex.Matches( body, @"[A-Za-z_][A-Za-z0-9_'-]*(?:\.[A-Za-z0-9_'-]+)*" ).Select( item => item.Value ).ToHashSet( );
    }

    private static void RequireNixInput( List<string> errors, string attribute, HashSet<string> recipe, HashSet<string> arguments,
        HashSet<string> flake, bool native, string? reason = null )
    {
        var list = native ? "nativeBuildInputs" : "buildInputs";
        var suffix = reason is null ? string.Empty : $" (required by {reason})";
        if ( !recipe.Contains( attribute ) )
        {
            errors.Add( $"packaging/nixpkgs/package.nix: {list} is missing `{attribute}`{suffix}" );
        }
        if ( !arguments.Contains( attribute ) )
        {
            errors.Add( $"packaging/nixpkgs/package.nix: function arguments are missing `{attribute}`" );
        }
        if ( !flake.Contains( attribute ) )
        {
            errors.Add( $"flake.nix: {list} is missing `{attribute}`{suffix}" );
        }
    }

    private static Task<int> ConfigWriters( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check config-writers" );
        var errors = ConfigWriterAudit.Run( context.RepositoryRoot );
        Failures( context, errors, $"config-writer audit passed ({ConfigWriterAudit.LastScannedCount} sources)", "config-writer audit failed:" );
        return Task.FromResult( ExitCodes.Success );
    }

    private static Task<int> LegacyTools( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check legacy-tools" );
        var expected = StandaloneToolFiles.ToHashSet( StringComparer.Ordinal );
        var actual = Directory.EnumerateFiles( context.Path( RepositoryPaths.ToolsDirectory ), "*", SearchOption.TopDirectoryOnly )
            .Where( path => Path.GetExtension( path ) is ".sh" or ".py" )
            .Select( Path.GetFileName ).ToHashSet( StringComparer.Ordinal )!;
        var missing = expected.Except( actual ).Order( ).ToArray( );
        if ( missing.Length > 0 )
        {
            throw new ToolException( "Standalone fallback scripts are missing:\n" + string.Join( '\n', missing.Select( name => $"- {name}" ) ) );
        }
        var unlisted = actual.Except( expected ).Order( ).ToArray( );
        if ( unlisted.Length > 0 )
        {
            throw new ToolException( "Standalone fallback inventory is incomplete:\n" + string.Join( '\n', unlisted.Select( name => $"- {name}" ) ) );
        }

        foreach ( var name in StandaloneToolFiles.Where( name => name.EndsWith( ".sh", StringComparison.Ordinal ) ) )
        {
            var source = Files.Read( context.Path( RepositoryPaths.ToolsDirectory, name ) );
            var toolRedirect = $"dotnet run {RepositoryPaths.ToolsDirectory}/{RepositoryNames.ToolEntryFile}";
            if ( source.Contains( toolRedirect, StringComparison.OrdinalIgnoreCase ) || Regex.IsMatch( source, @"(?m)^\s*(?:exec\s+)?dotnet\b" ) )
            {
                throw new ToolException( $"Standalone fallback redirects to .NET: tools/{name}" );
            }
        }
        foreach ( var path in Directory.EnumerateFiles( context.Path( RepositoryPaths.ToolsDirectory, "csharp" ), "*.cs", SearchOption.AllDirectories ) )
        {
            var source = Files.Read( path );
            if ( Regex.IsMatch( source, "(?:ProcessRequest|context\\.Run)\\s*\\(\\s*\\\"(?:(?:ba|z)?sh|python(?:3(?:\\.\\d+)?)?)\\\"" ) )
            {
                throw new ToolException( $"C# automation invokes a shell or Python interpreter: {Path.GetRelativePath( context.RepositoryRoot, path )}" );
            }
        }
        context.Output.WriteLine( "Standalone fallback scripts are inventoried and C# does not invoke a shell or Python interpreter." );
        return Task.FromResult( ExitCodes.Success );
    }

    private static void Failures( ToolContext context, IReadOnlyCollection<string> errors, string success, string heading = "Check failed:" )
    {
        if ( errors.Count > 0 )
        {
            throw new ToolException( heading + "\n" + string.Join( '\n', errors.Select( error => $"- {error}" ) ) );
        }
        context.Output.WriteLine( success );
    }

    private static readonly IReadOnlyDictionary<string, string[]> SystemLibraries = new Dictionary<string, string[]>( StringComparer.Ordinal )
    {
        ["anyhow"] = [],
        ["cairo-rs"] = ["cairo"],
        ["flate2"] = [],
        ["getrandom"] = [],
        ["glib"] = [],
        ["gtk4"] = ["gtk4"],
        [RepositoryNames.Gtk4LayerShell] = [RepositoryNames.Gtk4LayerShell],
        ["input"] = ["libinput"],
        ["ksni"] = [],
        ["libc"] = [],
        ["log"] = [],
        ["pango"] = ["pango"],
        ["pangocairo"] = ["pango", "cairo"],
        ["png"] = [],
        ["schemars"] = [],
        ["serde"] = [],
        ["serde_ignored"] = [],
        ["serde_json"] = [],
        ["smithay-client-toolkit"] = ["libxkbcommon"],
        ["tempfile"] = [],
        ["tokio"] = [],
        ["toml"] = [],
        ["toml_edit"] = [],
        ["udev"] = ["udev"],
        ["unicode-segmentation"] = [],
        ["wayland-client"] = ["wayland"],
        ["wayland-protocols"] = [],
        ["wayland-protocols-wlr"] = [],
        ["xkbcommon"] = ["libxkbcommon"],
        ["zbus"] = [],
        ["zune-jpeg"] = [],
    };
}

internal static class ConfigWriterAudit
{
    internal static int LastScannedCount
    {
        get; private set;
    }

    internal static List<string> Run( string root )
    {
        var errors = new List<string>( );
        var sources = new[] { "src", "configurator/src" }.SelectMany( directory =>
            Directory.EnumerateFiles( Path.Combine( root, directory ), "*.rs", SearchOption.AllDirectories ) ).Order( ).ToArray( );
        LastScannedCount = sources.Length;
        var owners = new HashSet<string>( StringComparer.Ordinal )
        {
            "src/config/document.rs", "src/config/io.rs", "configurator/src/app/io.rs",
        };
        string[] primitives = ["save_with_backup", "write_config_text_atomic", "create_config_backup", "prepare_config_parent"];
        string[] writers = ["persist_keybinding_edit", "persist_preset_slot", "persist_quick_color"];
        const string expectedCaller = "src/backend/wayland/config_edits.rs";
        var callers = writers.ToDictionary( name => name, _ => new HashSet<string>( StringComparer.Ordinal ) );

        ScanSources( root, sources, owners, primitives, writers, callers, errors );
        ValidateWriterCallers( writers, expectedCaller, callers, errors );
        ValidateWriterDefinitions( root, primitives, writers, errors );
        return errors;
    }

    private static void ScanSources( string root, IEnumerable<string> sources, HashSet<string> owners,
        IEnumerable<string> primitives, IEnumerable<string> writers, Dictionary<string, HashSet<string>> callers, List<string> errors )
    {
        foreach ( var path in sources )
        {
            var relative = Path.GetRelativePath( root, path ).Replace( Path.DirectorySeparatorChar, '/' );
            if ( IsTestSource( relative ) )
            {
                continue;
            }
            var masked = RemoveCfgTestBlocks( StripRustCommentsAndStrings( Files.Read( path ) ) );
            foreach ( var primitive in primitives )
            {
                if ( !owners.Contains( relative ) && Regex.IsMatch( masked, $@"\b{primitive}\b" ) )
                {
                    errors.Add( $"{relative}: config write capability `{primitive}` outside the reviewed writers" );
                }
            }
            foreach ( var writer in writers )
            {
                if ( relative is not "src/config/io.rs" and not "src/config/mod.rs" && Regex.IsMatch( masked, $@"\b{writer}\b" ) )
                {
                    callers[writer].Add( relative );
                }
                if ( Regex.IsMatch( masked, $@"\b{writer}\s+as\s+\w+" ) )
                {
                    errors.Add( $"{relative}: renames config writer `{writer}`" );
                }
                if ( relative is not "src/config/io.rs" and not "src/config/mod.rs" && Regex.IsMatch( masked, $@"\b{writer}_at\b" ) )
                {
                    errors.Add( $"{relative}: production code names `{writer}_at`" );
                }
            }
        }
    }

    private static void ValidateWriterCallers( IEnumerable<string> writers, string expectedCaller,
        Dictionary<string, HashSet<string>> callers, List<string> errors )
    {
        foreach ( var writer in writers )
        {
            foreach ( var unexpected in callers[writer].Where( path => path != expectedCaller ) )
            {
                errors.Add( $"{unexpected}: unreviewed caller of `{writer}`" );
            }
            if ( !callers[writer].Contains( expectedCaller ) )
            {
                errors.Add( $"{expectedCaller}: expected to call `{writer}` but does not" );
            }
        }
    }

    private static void ValidateWriterDefinitions( string root, IEnumerable<string> primitives, IEnumerable<string> writers, List<string> errors )
    {
        var document = Files.Read( Path.Combine( root, "src/config/document.rs" ) );
        var io = Files.Read( Path.Combine( root, "src/config/io.rs" ) );
        if ( !document.Contains( "pub fn save_with_backup", StringComparison.Ordinal ) )
        {
            errors.Add( "src/config/document.rs: save_with_backup is gone or renamed" );
        }
        foreach ( var primitive in primitives.Where( primitive => primitive != "save_with_backup" ) )
        {
            if ( !io.Contains( $"pub(super) fn {primitive}", StringComparison.Ordinal ) )
            {
                errors.Add( $"src/config/io.rs: `{primitive}` is no longer pub(super)" );
            }
        }
        foreach ( var writer in writers )
        {
            if ( !io.Contains( $"pub fn {writer}", StringComparison.Ordinal ) )
            {
                errors.Add( $"src/config/io.rs: narrow config writer `{writer}` is gone" );
            }
            if ( !Regex.IsMatch( io, $@"#\[cfg\(test\)\]\s*pub\(crate\) fn {writer}_at\b" ) )
            {
                errors.Add( $"src/config/io.rs: `{writer}_at` is no longer a #[cfg(test)] pub(crate) fn" );
            }
        }
        if ( Regex.IsMatch( RemoveCfgTestBlocks( StripRustCommentsAndStrings( io ) ), @"\bauthored_config\b" ) )
        {
            errors.Add( "src/config/io.rs: a narrow writer reads authored_config()" );
        }
    }

    private static bool IsTestSource( string relative ) =>
        relative.StartsWith( "tests/", StringComparison.Ordinal ) || relative.Split( '/' ).Contains( "tests" ) ||
        Path.GetFileName( relative ) is "tests.rs" or "test_helpers.rs" or "test_support.rs" ||
        Path.GetFileName( relative ).StartsWith( "test_", StringComparison.Ordinal ) ||
        Path.GetFileName( relative ).EndsWith( "_tests.rs", StringComparison.Ordinal );

    internal static string RemoveCfgTestBlocks( string text )
    {
        var chars = text.ToCharArray( );
        foreach ( Match marker in Regex.Matches( text, @"#\[cfg\((?!\s*not\s*\(\s*test\s*\))(?=[^]]*\btest\b)[^]]*\)\]" ) )
        {
            var opening = text.IndexOfAny( ['{', ';'], marker.Index + marker.Length );
            if ( opening < 0 )
            {
                continue;
            }
            var end = opening + 1;
            if ( text[opening] == '{' )
            {
                var depth = 1;
                while ( end < text.Length && depth > 0 )
                {
                    if ( text[end] == '{' )
                    {
                        depth++;
                    }
                    else if ( text[end] == '}' )
                    {
                        depth--;
                    }

                    end++;
                }
            }
            for ( var index = marker.Index; index < end; index++ )
            {
                if ( chars[index] != '\n' )
                {
                    chars[index] = ' ';
                }
            }
        }
        return new string( chars );
    }

    internal static string StripRustCommentsAndStrings( string text )
    {
        var output = text.ToCharArray( );
        var index = 0;
        var blockDepth = 0;
        while ( index < text.Length )
        {
            if ( blockDepth == 0 && StartsWith( text, index, '/', '/' ) )
            {
                MaskLineComment( text, output, ref index );
            }
            else if ( StartsWith( text, index, '/', '*' ) )
            {
                MaskPair( output, ref index );
                blockDepth++;
            }
            else if ( blockDepth > 0 )
            {
                MaskBlockCommentCharacter( text, output, ref index, ref blockDepth );
            }
            else if ( text[index] == '"' )
            {
                MaskString( text, output, ref index );
            }
            else
            {
                index++;
            }
        }
        return new string( output );
    }

    private static bool StartsWith( string text, int index, char first, char second ) =>
        index + 1 < text.Length && text[index] == first && text[index + 1] == second;

    private static void MaskLineComment( string text, char[] output, ref int index )
    {
        while ( index < text.Length && text[index] != '\n' )
        {
            output[index++] = ' ';
        }
    }

    private static void MaskBlockCommentCharacter( string text, char[] output, ref int index, ref int blockDepth )
    {
        if ( StartsWith( text, index, '*', '/' ) )
        {
            MaskPair( output, ref index );
            blockDepth--;
            return;
        }

        if ( text[index] != '\n' )
        {
            output[index] = ' ';
        }
        index++;
    }

    private static void MaskPair( char[] output, ref int index )
    {
        output[index++] = ' ';
        output[index++] = ' ';
    }

    private static void MaskString( string text, char[] output, ref int index )
    {
        output[index++] = ' ';
        while ( index < text.Length )
        {
            var character = text[index];
            if ( character != '\n' )
            {
                output[index] = ' ';
            }
            index++;

            if ( character == '\\' && index < text.Length )
            {
                if ( text[index] != '\n' )
                {
                    output[index] = ' ';
                }
                index++;
                continue;
            }

            if ( character == '"' )
            {
                return;
            }
        }
    }
}
