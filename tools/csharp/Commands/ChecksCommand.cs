using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal static class ChecksCommand
{
    private const string LibraryFilePrefix = "lib";
    // The complete local gate: it runs the C# checks, so it is the one script that needs .NET.
    private const string GateEntryPoint = "lint-and-test.sh";
    // C# that hands a command to bash, sh, or zsh, by name or path: passed straight to
    // ProcessRequest or context.Run, or assigned to a constant or variable that a launch uses.
    private const string ShellLaunchPattern =
        "(?:(?:ProcessRequest|context\\.Run)\\s*\\(\\s*|=\\s*)\"(?:/usr)?(?:/bin/)?(?:ba|z)?sh\"";
    private static readonly string[] ShellToolFiles =
    [
        "build-package-repos.sh", "build.sh", "check-arch-installer-manifest.sh", "fetch-all-deps.sh",
        "install-configurator.sh", "install-gtk4-layer-shell.sh", "install.sh", GateEntryPoint, "package.sh",
        "reload-daemon.sh", "run.sh", "set-portal-shortcut.sh", "test-gtk-widgets.sh",
        "test-package-repo-layout.sh", "test-release-packaging.sh", "test.sh", "update-aur-from-manifest.sh",
        "update-aur.sh", "verify-static-gtk4-layer-shell.sh",
    ];

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Check, CommandNames.RustSourceCoverage, "Verify every Rust source is compiled.", SideEffect.ReadOnly, RustSourceCoverage ),
        new( CommandAreas.Check, CommandNames.NixpkgsRecipe, "Check Cargo native dependencies against Nix.", SideEffect.ReadOnly, NixpkgsRecipe ),
        new( CommandAreas.Check, CommandNames.LegacyTools, "Verify the shell tool inventory and that C# never launches a shell.",
            SideEffect.ReadOnly, LegacyTools ),
    ];

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
        var flake = Files.Read( context.Path( RepositoryNames.FlakeFile ) );
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

    private static Task<int> LegacyTools( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check legacy-tools" );
        var expected = ShellToolFiles.ToHashSet( StringComparer.Ordinal );
        var actual = Directory.EnumerateFiles( context.Path( RepositoryPaths.ToolsDirectory ), "*.sh", SearchOption.TopDirectoryOnly )
            .Select( Path.GetFileName ).ToHashSet( StringComparer.Ordinal )!;

        var missing = expected.Except( actual ).Order( ).ToArray( );
        if ( missing.Length > 0 )
        {
            throw new ToolException( "Shell tools are missing:\n" + string.Join( '\n', missing.Select( name => $"- {name}" ) ) );
        }
        var unlisted = actual.Except( expected ).Order( ).ToArray( );
        if ( unlisted.Length > 0 )
        {
            var names = string.Join( '\n', unlisted.Select( name => $"- {name}" ) );
            throw new ToolException( "Shell tool inventory is incomplete:\n" + names );
        }

        // The complete local gate runs the C# checks, which exist only here; every
        // other shell tool stays usable without .NET.
        foreach ( var name in ShellToolFiles.Where( name => name != GateEntryPoint ) )
        {
            var source = Files.Read( context.Path( RepositoryPaths.ToolsDirectory, name ) );
            var toolRedirect = $"dotnet run {RepositoryPaths.ToolsDirectory}/{RepositoryNames.ToolEntryFile}";
            if ( source.Contains( toolRedirect, StringComparison.OrdinalIgnoreCase ) || Regex.IsMatch( source, @"(?m)^\s*(?:exec\s+)?dotnet\b" ) )
            {
                throw new ToolException( $"Shell tool redirects to .NET: tools/{name}" );
            }
        }

        // Any Python spelling, an interpreter launch included, fails tests/repository_guards/no_python.rs.
        foreach ( var path in Directory.EnumerateFiles( context.Path( RepositoryPaths.ToolsDirectory, "csharp" ), "*.cs", SearchOption.AllDirectories ) )
        {
            if ( Regex.IsMatch( Files.Read( path ), ShellLaunchPattern ) )
            {
                throw new ToolException( $"C# automation launches a shell: {Path.GetRelativePath( context.RepositoryRoot, path )}" );
            }
        }

        context.Output.WriteLine( "Shell tools are inventoried and usable without .NET, and C# does not launch a shell." );
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
