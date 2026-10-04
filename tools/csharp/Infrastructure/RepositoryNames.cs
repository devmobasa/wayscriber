namespace Wayscriber.Tools;

internal static class RepositoryNames
{
    public const string MainPackage = "wayscriber";
    public const string BinaryPackage = "wayscriber-bin";
    public const string ConfiguratorPackage = "wayscriber-configurator";
    public const string Gtk4LayerShell = "gtk4-layer-shell";
    public const string FlakeFile = "flake.nix";
    public const string GlobalJsonFile = "global.json";
    public const string ReadmeFile = "README.md";
    public const string MainPackageConfigFile = "package.wayscriber.yaml";
    public const string ConfiguratorPackageConfigFile = "package.configurator.yaml";
    public const string PackageBuildFile = "PKGBUILD";
    public const string SourceInfoFile = ".SRCINFO";
    public const string UserServiceFile = "wayscriber.service";
    public const string ToolEntryFile = "wayscriber.cs";
    public const string ToolTestEntryFile = "wayscriber.tests.cs";
    public const string ReleaseArchiveVersionPrefix = "-v";
    public const string ReleaseArchiveSuffix = "-linux-x86_64";
    public const string ReleaseArchiveExtension = ".tar.gz";

    // `<package>-v<version>-linux-x86_64`: a release archive's top directory, and its name without the extension.
    public static string ReleaseArchiveRoot( string package, string version ) =>
        $"{package}{ReleaseArchiveVersionPrefix}{version}{ReleaseArchiveSuffix}";

    public static string ReleaseArchive( string package, string version ) =>
        ReleaseArchiveRoot( package, version ) + ReleaseArchiveExtension;
}
