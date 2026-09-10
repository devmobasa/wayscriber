namespace Wayscriber.Tools;

internal static class ExitCodes
{
    public const int Success = 0;
    public const int Failure = 1;
    public const int InvalidArguments = 2;
    public const int SoftwareError = 70;
    public const int CommandNotFound = 127;
    public const int Canceled = 130;
}

internal static class CommandAreas
{
    public const string Assets = "assets";
    public const string Aur = "aur";
    public const string Check = "check";
    public const string ContinuousIntegration = "ci";
    public const string Desktop = "desktop";
    public const string Development = "dev";
    public const string Elf = "elf";
    public const string Install = "install";
    public const string Native = "native";
    public const string Package = "package";
    public const string Release = "release";
    public const string Report = "report";
    public const string Version = "version";
}

internal static class CommandNames
{
    public const string App = "app";
    public const string Build = "build";
    public const string BuildLinkage = "build-linkage";
    public const string BuildRepositories = "build-repositories";
    public const string Bump = "bump";
    public const string Check = "check";
    public const string CheckArchInstaller = "check-arch-installer";
    public const string CheckLiveArchInstaller = "check-live-arch-installer";
    public const string Clone = "clone";
    public const string CodeHealth = "code-health";
    public const string ConfigWriters = "config-writers";
    public const string Configurator = "configurator";
    public const string ConfigureGit = "configure-git";
    public const string CreateTag = "create-tag";
    public const string DeployPackageRepositories = "deploy-package-repositories";
    public const string Emit = "emit";
    public const string Fetch = "fetch";
    public const string FormatAndLint = "format-and-lint";
    public const string GtkWidgets = "gtk-widgets";
    public const string Help = "help";
    public const string InstallDependencies = "install-dependencies";
    public const string InstallGtk4LayerShell = "install-gtk4-layer-shell";
    public const string InstallNfpm = "install-nfpm";
    public const string LegacyTools = "legacy-tools";
    public const string LintAndTest = "lint-and-test";
    public const string NixInstantiation = "nix-instantiation";
    public const string NixpkgsRecipe = "nixpkgs-recipe";
    public const string NixVersions = "nix-versions";
    public const string PrepareGtk4LayerShell = "prepare-gtk4-layer-shell";
    public const string PrepareSsh = "prepare-ssh";
    public const string ProcessSites = "process-sites";
    public const string PublishTag = "publish-tag";
    public const string ReloadDaemon = "reload-daemon";
    public const string RequireEnvironment = "require-environment";
    public const string ResolveVersion = "resolve-version";
    public const string RustSourceCoverage = "rust-source-coverage";
    public const string SetPortalShortcut = "set-portal-shortcut";
    public const string SharedDependencies = "shared-dependencies";
    public const string SmokeUbuntu = "smoke-ubuntu";
    public const string SourceChecksum = "source-checksum";
    public const string Test = "test";
    public const string Update = "update";
    public const string VerifyArtifacts = "verify-artifacts";
    public const string VerifyDynamic = "verify-dynamic";
    public const string VerifyStatic = "verify-static";
}

internal static class CommandLineOptions
{
    public const string AllFeatures = "--all-features";
    public const string AllTargets = "--all-targets";
    public const string Binaries = "--bins";
    public const string ChangeDirectory = "-C";
    public const string EndOfOptions = "--";
    public const string Help = "--help";
    public const string Locked = "--locked";
    public const string NoDefaultFeatures = "--no-default-features";
    public const string NoRestore = "--no-restore";
    public const string Release = "--release";
    public const string ShortHelp = "-h";
    public const string SingleTestThread = "--test-threads=1";
    public const string Version = "--version";
    public const string VerifyNoChanges = "--verify-no-changes";
    public const string Workspace = "--workspace";
}

internal static class PackageChannels
{
    public const string Binary = "bin";
    public const string Configurator = "configurator";
    public const string Source = "source";
}

internal static class PackageFormats
{
    public const string Apt = "apt";
    public const string Debian = "deb";
    public const string Rpm = "rpm";
    public const string Tar = "tar";
}

internal static class AutostartModes
{
    public const string Hyprland = "hyprland";
    public const string None = "none";
    public const string Systemd = "systemd";
}

internal static class NativeLibraryModes
{
    public const string Both = "both";
    public const string Dynamic = "dynamic";
    public const string Shared = "shared";
    public const string Static = "static";
}

internal static class InstallArguments
{
    public const string DataFile = "-Dm644";
    public const string ExecutableFile = "-Dm755";
}

internal static class EnvironmentVariables
{
    public const string ArtifactRoot = "ARTIFACT_ROOT";
    public const string AurBinaryDirectory = "AUR_BIN_DIR";
    public const string AurConfiguratorDirectory = "AUR_CONFIG_DIR";
    public const string AurGitEmail = "AUR_GIT_EMAIL";
    public const string AurGitUserName = "AUR_GIT_USERNAME";
    public const string AurSourceArchiveSha256 = "AUR_SOURCE_ARCHIVE_SHA256";
    public const string AurSourceDirectory = "AUR_SOURCE_DIR";
    public const string AurSshKnownHosts = "AUR_SSH_KNOWN_HOSTS";
    public const string AurSshPassphrase = "AUR_SSH_PASSPHRASE";
    public const string AurSshPrivateKey = "AUR_SSH_PRIVATE_KEY";
    public const string DebArchitecture = "DEB_ARCH";
    public const string DebComponent = "DEB_COMPONENT";
    public const string DebSuite = "DEB_SUITE";
    public const string DeployHost = "DEPLOY_HOST";
    public const string DeployPath = "DEPLOY_PATH";
    public const string DeployUser = "DEPLOY_USER";
    public const string Display = "DISPLAY";
    public const string Disabled = "0";
    public const string Enabled = "1";
    public const string Force = "force";
    public const string ForceBuild = "FORCE_BUILD";
    public const string Formats = "FORMATS";
    public const string GdkBackend = "GDK_BACKEND";
    public const string GitHubEnvironment = "GITHUB_ENV";
    public const string GitHubOutput = "GITHUB_OUTPUT";
    public const string GitHubRefName = "GITHUB_REF_NAME";
    public const string GitHubStepSummary = "GITHUB_STEP_SUMMARY";
    public const string GitSshCommand = "GIT_SSH_COMMAND";
    public const string GnuPgHome = "GNUPGHOME";
    public const string GpgKeyId = "GPG_KEY_ID";
    public const string GpgPassphrase = "GPG_PASSPHRASE";
    public const string GpgPrivateKeyBase64 = "GPG_PRIVATE_KEY_B64";
    public const string Gtk4LayerShellLibraryMode = "GTK4_LAYER_SHELL_LIBRARY_MODE";
    public const string Gtk4LayerShellPrefix = "GTK4_LAYER_SHELL_PREFIX";
    public const string Gtk4LayerShellSystemPrefix = "GTK4_LAYER_SHELL_SYSTEM_PREFIX";
    public const string GtkAccessibility = "GTK_A11Y";
    public const string Home = "HOME";
    public const string LibraryPath = "LD_LIBRARY_PATH";
    public const string NfpmConfiguratorConfig = "NFPM_CONFIG_CONFIG";
    public const string NfpmMainConfig = "NFPM_CONFIG_MAIN";
    public const string OutputRoot = "OUTPUT_ROOT";
    public const string Path = "PATH";
    public const string PackageConfigurator = "PACKAGE_CONFIGURATOR";
    public const string PackageRepositorySshKey = "PACKAGE_REPO_SSH_KEY";
    public const string PackageRepositorySshKnownHosts = "PACKAGE_REPO_SSH_KNOWN_HOSTS";
    public const string PackageConfigPath = "PKG_CONFIG_PATH";
    public const string RepositoryLabel = "REPO_LABEL";
    public const string RepositoryOrigin = "REPO_ORIGIN";
    public const string RpmArchitecture = "RPM_ARCH";
    public const string RunnerTemporary = "RUNNER_TEMP";
    public const string SignRpms = "SIGN_RPMS";
    public const string Skip = "SKIP";
    public const string SkipBuild = "SKIP_BUILD";
    public const string SshAgentProcessId = "SSH_AGENT_PID";
    public const string SshAskpass = "SSH_ASKPASS";
    public const string SshAskpassRequire = "SSH_ASKPASS_REQUIRE";
    public const string SshAuthSocket = "SSH_AUTH_SOCK";
    public const string SystemGtk4LayerShellLink = "SYSTEM_DEPS_GTK4_LAYER_SHELL_0_LINK";
    public const string Version = "VERSION";
    public const string WaylandDisplay = "WAYLAND_DISPLAY";
    public const string WayscriberDataDirectory = "WAYSCRIBER_DATA_DIR";
    public const string WayscriberInstallDirectory = "WAYSCRIBER_INSTALL_DIR";
    public const string WayscriberReleaseVersion = "WAYSCRIBER_RELEASE_VERSION";
    public const string WayscriberRequireGtkTests = "WAYSCRIBER_REQUIRE_GTK_TESTS";
    public const string WayscriberSshAskpass = "WAYSCRIBER_SSH_ASKPASS";
    public const string XdgConfigHome = "XDG_CONFIG_HOME";
    public const string XdgRuntimeDirectory = "XDG_RUNTIME_DIR";
}

internal static class Programs
{
    public const string AptFileArchive = "apt-ftparchive";
    public const string AptGet = "apt-get";
    public const string Cargo = "cargo";
    public const string CreateRpmRepository = "createrepo_c";
    public const string DbusRunSession = "dbus-run-session";
    public const string Docker = "docker";
    public const string Dotnet = "dotnet";
    public const string DpkgDeb = "dpkg-deb";
    public const string Find = "find";
    public const string Git = "git";
    public const string Gpg = "gpg";
    public const string GtkUpdateIconCache = "gtk-update-icon-cache";
    public const string Install = "install";
    public const string LdConfig = "ldconfig";
    public const string Makepkg = "makepkg";
    public const string Meson = "meson";
    public const string Nfpm = "nfpm";
    public const string Nix = "nix";
    public const string Nm = "nm";
    public const string Pacman = "pacman";
    public const string Pgrep = "pgrep";
    public const string Pkill = "pkill";
    public const string PkgConfig = "pkg-config";
    public const string ReadElf = "readelf";
    public const string Remove = "rm";
    public const string Rpm = "rpm";
    public const string RpmSign = "rpmsign";
    public const string Rsync = "rsync";
    public const string Setsid = "setsid";
    public const string Ssh = "ssh";
    public const string SshAdd = "ssh-add";
    public const string SshAgent = "ssh-agent";
    public const string SshKeyScan = "ssh-keyscan";
    public const string Sleep = "sleep";
    public const string Strip = "strip";
    public const string Sudo = "sudo";
    public const string SystemControl = "systemctl";
    public const string Tar = "tar";
    public const string Test = "test";
    public const string UpdateDesktopDatabase = "update-desktop-database";
    public const string Wayscriber = "wayscriber";
    public const string Weston = "weston";
    public const string Which = "which";
}

internal static class RepositoryPaths
{
    public const string CargoLock = "Cargo.lock";
    public const string CargoManifest = "Cargo.toml";
    public const string ConfiguratorCargoManifest = "configurator/Cargo.toml";
    public const string ConfiguratorDirectory = "configurator";
    public const string DebugDirectory = "debug";
    public const string PackagingDirectory = "packaging";
    public const string ReleaseDirectory = "release";
    public const string TargetDirectory = "target";
    public const string ToolsDirectory = "tools";
}

internal static class ToolMessages
{
    public const string Canceled = "Canceled.";
    public const string HomeNotSet = "HOME is not set.";
    public const string UnexpectedFailurePrefix = "wayscriber tool failed: ";
}

internal static class HashingConstants
{
    public const int Sha256HexLength = 64;
    public const string Sha256HexPattern = "^[0-9a-fA-F]{64}$";
}

internal static class PackagingPlatform
{
    public const string MaximumGlibcVersion = "2.39";
    public const string MinimumGtkVersion = "4.12";
    public const string UbuntuImage = "ubuntu:24.04";
    public const string UbuntuRunner = "ubuntu-24.04";
}
