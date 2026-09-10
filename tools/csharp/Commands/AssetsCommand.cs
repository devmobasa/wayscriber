using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using YamlDotNet.Core;
using YamlDotNet.RepresentationModel;

namespace Wayscriber.Tools;

internal static class AssetsCommand
{
    private const int OctalRadix = 8;
    private const int DecimalRadix = 10;
    private const int DesktopAssetUnixMode = 420;
    private const string DesktopAssetMode = "0644";

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Assets, CommandNames.Emit, "Emit AUR desktop asset recipe JSON.", SideEffect.ReadOnly, Emit ),
        new( CommandAreas.Assets, CommandNames.Check, "Validate desktop assets in package manifests.", SideEffect.ReadOnly, Check ),
    ];

    private static Task<int> Emit( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "assets emit" );
        context.Output.WriteLine( CreateRecipe( context.RepositoryRoot ).ToJsonString( ) );
        return Task.FromResult( ExitCodes.Success );
    }

    private static Task<int> Check( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "assets check" );
        _ = CreateRecipe( context.RepositoryRoot );
        context.Output.WriteLine( "AUR desktop asset manifests passed." );
        return Task.FromResult( ExitCodes.Success );
    }

    internal static JsonObject CreateRecipe( string root )
    {
        try
        {
            var main = ReadAssets( root, RepositoryNames.MainPackage, "package.wayscriber.yaml" );
            var configurator = ReadAssets( root, RepositoryNames.ConfiguratorPackage, "package.configurator.yaml" );
            return new JsonObject
            {
                ["desktop_path_pattern"] = string.Join( "|", DesktopPrefixes.Select( Regex.Escape ) ),
                [PackageChannels.Source] = Recipe( "wayscriber source", "Wayscriber", "target/release/wayscriber", RepositoryNames.MainPackage, main, false ),
                [PackageChannels.Binary] = Recipe( "wayscriber bin", "Wayscriber", "${srcdir_tmp}/usr/bin/wayscriber", RepositoryNames.MainPackage, main, true ),
                [PackageChannels.Configurator] = Recipe( RepositoryNames.ConfiguratorPackage, "Wayscriber configurator", "target/release/wayscriber-configurator",
                    RepositoryNames.ConfiguratorPackage, configurator, false ),
            };
        }
        catch ( Exception error ) when ( error is IOException or UnauthorizedAccessException or YamlException or InvalidDataException )
        {
            throw new ToolException( $"Desktop asset manifest error: {error.Message}" );
        }
    }

    private static List<(string Source, string Destination)> ReadAssets( string root, string package, string file )
    {
        using var reader = File.OpenText( Path.Combine( root, RepositoryPaths.PackagingDirectory, file ) );
        var yaml = new YamlStream( );
        yaml.Load( reader );
        if ( yaml.Documents.Count != 1 || yaml.Documents[0].RootNode is not YamlMappingNode document ||
            !document.Children.TryGetValue( new YamlScalarNode( "contents" ), out var contentNode ) || contentNode is not YamlSequenceNode contents )
        {
            throw new InvalidDataException( $"{file}: expected one package contents sequence" );
        }

        var assets = new List<(string, string)>( );
        var destinations = new HashSet<string>( StringComparer.Ordinal );
        foreach ( var node in contents.Children )
        {
            if ( node is not YamlMappingNode entry )
            {
                throw new InvalidDataException( $"{file}: invalid content entry" );
            }
            var destination = Scalar( entry, "dst" );
            if ( !DesktopPrefixes.Any( prefix => destination.StartsWith( prefix, StringComparison.Ordinal ) ) )
            {
                continue;
            }
            var source = Scalar( entry, "src" );
            ValidateAsset( root, file, entry, source, destination );
            if ( !destinations.Add( destination ) )
            {
                throw new InvalidDataException( $"{file}: duplicate desktop destination {destination}" );
            }
            assets.Add( (source, destination) );
        }

        if ( !destinations.Contains( $"/usr/share/applications/{package}.desktop" ) ||
            !destinations.Any( path => path.StartsWith( "/usr/share/icons/", StringComparison.Ordinal ) ) )
        {
            throw new InvalidDataException( $"{file}: launcher and icons are required" );
        }
        return assets;
    }

    private static void ValidateAsset( string root, string file, YamlMappingNode entry, string source, string destination )
    {
        if ( !Regex.IsMatch( source, @"^packaging/[A-Za-z0-9_./-]+$" ) ||
            !Regex.IsMatch( destination, @"^/usr/share/[A-Za-z0-9_./-]+$" ) ||
            source.Split( '/' ).Contains( ".." ) || destination.Split( '/' ).Contains( ".." ) )
        {
            throw new InvalidDataException( $"{file}: unsupported asset path {source} -> {destination}" );
        }
        if ( !entry.Children.TryGetValue( new YamlScalarNode( "file_info" ), out var infoNode ) || infoNode is not YamlMappingNode info )
        {
            throw new InvalidDataException( $"{file}: desktop asset {destination} has no file_info mapping" );
        }
        var modeError = $"{file}: desktop asset {destination} must have mode {DesktopAssetMode} " +
            $"(plain YAML integer: {DesktopAssetMode}, 0o644, or {DesktopAssetUnixMode})";
        if ( !info.Children.TryGetValue( new YamlScalarNode( "mode" ), out var modeNode ) ||
            modeNode is not YamlScalarNode { Style: ScalarStyle.Plain, Value: { } mode } || modeNode.Tag.ToString( ) == "tag:yaml.org,2002:str" )
        {
            throw new InvalidDataException( modeError );
        }
        var explicitOctal = mode.StartsWith( "0o", StringComparison.OrdinalIgnoreCase );
        var digits = explicitOctal ? mode[2..] : mode;
        var numberBase = explicitOctal || mode.StartsWith( '0' ) ? OctalRadix : DecimalRadix;
        try
        {
            if ( Convert.ToInt32( digits, numberBase ) != DesktopAssetUnixMode )
            {
                throw new InvalidDataException( modeError );
            }
        }
        catch ( Exception error ) when ( error is FormatException or OverflowException or ArgumentException )
        {
            throw new InvalidDataException( $"{modeError}; got '{mode}'", error );
        }
        if ( !File.Exists( Path.Combine( root, source ) ) )
        {
            throw new InvalidDataException( $"{file}: missing desktop asset {source}" );
        }
    }

    private static string Scalar( YamlMappingNode node, string key )
    {
        if ( !node.Children.TryGetValue( new YamlScalarNode( key ), out var value ) || value is not YamlScalarNode { Value: { } text } )
        {
            throw new InvalidDataException( $"Expected scalar {key}" );
        }
        return text;
    }

    private static JsonObject Recipe( string label, string title, string binary, string package,
        List<(string Source, string Destination)> assets, bool fromArchive )
    {
        var lines = new JsonArray( );
        var destinations = new JsonArray( );
        foreach ( var (source, destination) in assets )
        {
            var input = fromArchive ? $"\"${{srcdir_tmp}}{destination}\"" : source;
            lines.Add( JsonValue.Create( $"    install -Dm644 {input} \"$pkgdir{destination}\"" ) );
            destinations.Add( JsonValue.Create( destination ) );
        }
        return new JsonObject
        {
            ["label"] = label,
            ["marker"] = $"# {title} desktop integration",
            ["end_marker"] = $"# End {title} desktop integration",
            ["anchor"] = $"    install -Dm755 \"{binary}\" \"$pkgdir/usr/bin/{package}\"",
            ["lines"] = lines,
            ["destinations"] = destinations,
        };
    }

    private static readonly string[] DesktopPrefixes = ["/usr/share/applications/", "/usr/share/icons/", "/usr/share/pixmaps/"];
}
