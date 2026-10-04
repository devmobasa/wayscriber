using Xunit;

namespace Wayscriber.Tools.Tests;

// Each case edits one independently editable metadata surface of a copied repository,
// so a partial floor bump, stale recipe, or pinned install example cannot pass `version check`.
public sealed class VersionConsistencyTests
{
    private const string CargoPlaceholder = "{cargo}";
    private const string FixedChecksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    private const string ShellAurUpdater = "tools/update-aur-from-manifest.sh";
    private const string ReleaseWorkflow = ".github/workflows/build-packages.yml";
    private const string PackageRunner =
        "    # This runner defines the oldest supported release ABI (glibc 2.39).\n    runs-on: ubuntu-24.04";
    private const string StaleReadme = "goes stale on the next release; use a RELEASE_TAG placeholder or link to /releases/latest";

    [Fact]
    public async Task CopiedRepositoryMetadataPasses( )
    {
        using var fixture = new VersionMetadataFixture( );

        var output = await fixture.Check( );

        var cargo = fixture.CargoVersion;
        Assert.Equal( $"Version consistency OK: Cargo={cargo}, packaging={cargo}, checksum=SKIP, libadwaita=1.4\n", output );
    }

    [Theory]
    [InlineData( """{"sdk":{"version":"11.0.100-rc.1.26425.128","rollForward":"disable","allowPrerelease":true}}""" )]
    [InlineData( """{"sdk":{"version":" 11.0.100-rc.1.26425.128 ","rollForward":"disable","allowPrerelease":true}}""" )]
    public async Task CompactGlobalJsonWithTheReviewedSdkPasses( string globalJson )
    {
        using var fixture = new VersionMetadataFixture( );
        fixture.Write( "global.json", globalJson );

        Assert.StartsWith( "Version consistency OK:", await fixture.Check( ), StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( """{"sdk":{}}""", "global.json SDK metadata is invalid" )]
    [InlineData( "not json", "global.json SDK metadata is invalid" )]
    [InlineData( """{"sdk":{"version":" ","rollForward":"disable","allowPrerelease":true}}""",
        "global.json SDK metadata is invalid: sdk.version must be a non-empty string" )]
    [InlineData( """{"sdk":{"version":11,"rollForward":"disable","allowPrerelease":true}}""",
        "global.json SDK metadata is invalid: sdk.version must be a non-empty string" )]
    [InlineData( """{"sdk":{"version":"11.0.100-rc.1.26425.127","rollForward":"disable","allowPrerelease":true}}""",
        "global.json SDK: expected 11.0.100-rc.1.26425.128, got 11.0.100-rc.1.26425.127" )]
    [InlineData( """{"sdk":{"version":"11.0.100-rc.1.26425.128","rollForward":"latestPatch","allowPrerelease":false}}""",
        "global.json SDK rollForward must be disable" )]
    [InlineData( """{"sdk":{"version":"11.0.100-rc.1.26425.128","rollForward":"latestPatch","allowPrerelease":false}}""",
        "global.json SDK allowPrerelease must be true" )]
    [InlineData( """{"sdk":{"version":"11.0.100-rc.1.26425.128","rollForward":"disable","allowPrerelease":"true"}}""",
        "global.json SDK allowPrerelease must be true" )]
    public async Task GlobalJsonMustPinTheReviewedSdk( string globalJson, string expected )
    {
        using var fixture = new VersionMetadataFixture( );
        fixture.Write( "global.json", globalJson );

        Assert.Contains( expected, await fixture.CheckExpectingFailure( ), StringComparison.Ordinal );
    }

    [Theory]
    // One supported libadwaita floor governs compilation and every package channel.
    [InlineData( "configurator/Cargo.toml", """features = ["v1_4"]""", """features = ["v1_5"]""",
        "configurator/Cargo.toml libadwaita features: expected ['v1_4'], got ['v1_5']" )]
    [InlineData( "configurator/Cargo.toml", """features = ["v1_4"]""", """features = ["v1_4", 4]""",
        "configurator/Cargo.toml: libadwaita features must be a string list" )]
    [InlineData( "configurator/Cargo.toml", """libadwaita = { version = "0.9", features = ["v1_4"] }""", "libadwaita = \"0.9\"",
        "configurator/Cargo.toml: missing structured libadwaita dependency" )]
    [InlineData( "configurator/Cargo.toml", """libadwaita = { version = "0.9", features = ["v1_4"] }""",
        """libadwaita = { version = "0.9" }""", "configurator/Cargo.toml libadwaita features: expected ['v1_4'], got []" )]
    [InlineData( "packaging/package.configurator.yaml", "libadwaita-1-0 (>= 1.4)", "libadwaita-1-0 (>= 1.5)",
        "configurator deb libadwaita floor: expected 1.4, got 1.5" )]
    [InlineData( "packaging/package.configurator.yaml", "libadwaita >= 1.4", "libadwaita >= 1.5",
        "configurator rpm libadwaita floor: expected 1.4, got 1.5" )]
    [InlineData( "packaging/PKGBUILD", "'libadwaita>=1.4'", "'libadwaita>=1.5'",
        "packaging/PKGBUILD libadwaita floor: expected 1.4, got 1.5" )]
    [InlineData( "packaging/.SRCINFO", "depends = libadwaita>=1.4", "depends = libadwaita>=1.5",
        "packaging/.SRCINFO libadwaita floor: expected 1.4, got 1.5" )]
    // The standalone AUR updater generates and validates recipes with its own literal floor.
    [InlineData( ShellAurUpdater, "ensure_runtime_dependency 'libadwaita>=1.4' gcc-libs",
        "ensure_runtime_dependency 'libadwaita>=1.5' gcc-libs", "AUR updater generated libadwaita floor: expected 1.4, got 1.5" )]
    [InlineData( ShellAurUpdater, """'libadwaita>=1.4'[[:space:]]*$" PKGBUILD""", """'libadwaita>=1.5'[[:space:]]*$" PKGBUILD""",
        "AUR updater PKGBUILD validation floor: expected 1.4, got 1.5" )]
    [InlineData( ShellAurUpdater, "depends = libadwaita>=1.4' .SRCINFO", "depends = libadwaita>=1.5' .SRCINFO",
        "AUR updater .SRCINFO validation floor: expected 1.4, got 1.5" )]
    [InlineData( ShellAurUpdater, "    ensure_runtime_dependency 'libadwaita>=1.4' gcc-libs\n",
        "    ensure_runtime_dependency 'libadwaita>=1.4' gcc-libs\n    ensure_runtime_dependency 'libadwaita>=1.4' gcc-libs\n",
        "AUR updater generated libadwaita floor: expected one libadwaita floor, found 2" )]
    // The package job's runner defines the binary ABI and needs a reviewed floor contract.
    [InlineData( ReleaseWorkflow, PackageRunner, "    runs-on: ubuntu-26.04",
        "release package runner libadwaita floor: no reviewed contract for ubuntu-26.04" )]
    [InlineData( ReleaseWorkflow, PackageRunner, PackageRunner + "\n    runs-on: ubuntu-24.04",
        ".github/workflows/build-packages.yml package job: expected one literal runs-on value, found 2" )]
    [InlineData( ReleaseWorkflow, "jobs:\n  package:\n", "jobs:\n  packages:\n",
        ".github/workflows/build-packages.yml: missing package job" )]
    // Workspace versions, lockfile, and the packaging template follow Cargo.toml.
    [InlineData( "Cargo.toml", "version = \"{cargo}\"\n", "version = \"{cargo}-rc.1\"\n",
        "Cargo.toml version has unsupported format: {cargo}-rc.1" )]
    [InlineData( "configurator/Cargo.toml", "version = \"{cargo}\"\n", "version = \"0.0.1\"\n",
        "configurator/Cargo.toml: expected {cargo}, got 0.0.1" )]
    [InlineData( "Cargo.lock", "name = \"wayscriber\"\nversion = \"{cargo}\"", "name = \"wayscriber\"\nversion = \"0.0.1\"",
        "Cargo.lock wayscriber: expected {cargo}, got 0.0.1" )]
    [InlineData( "packaging/PKGBUILD", "pkgver={cargo}\n", "pkgver=0.0.1\n", "packaging/PKGBUILD pkgver: expected {cargo}, got 0.0.1" )]
    [InlineData( "packaging/.SRCINFO", "\tpkgver = {cargo}\n", "", "packaging/.SRCINFO pkgver: expected {cargo}, got missing" )]
    // Repo packaging metadata is a template; automation writes the real checksum after tagging.
    [InlineData( "packaging/PKGBUILD", "sha256sums=('SKIP')", "sha256sums=('" + FixedChecksum + "')",
        "packaging/PKGBUILD sha256sums: expected SKIP template checksum, got fixed SHA " + FixedChecksum +
        "; release/AUR automation writes the real checksum after the tag exists" )]
    [InlineData( "packaging/PKGBUILD", "sha256sums=('SKIP')", "sha256sums=('SKIP!')",
        "packaging/PKGBUILD sha256sums: expected SKIP template checksum, got SKIP!" )]
    [InlineData( "packaging/PKGBUILD", "sha256sums=('SKIP')", "sha256sums=('SKIP' 'SKIP')",
        "packaging/PKGBUILD sha256sums: expected SKIP template checksum, got SKIP, SKIP" )]
    [InlineData( "packaging/PKGBUILD", "sha256sums=('SKIP')", "",
        "packaging/PKGBUILD sha256sums: expected SKIP template checksum, got missing" )]
    [InlineData( "packaging/.SRCINFO", "sha256sums = SKIP", "sha256sums = " + FixedChecksum,
        "packaging/.SRCINFO sha256sums: expected SKIP template checksum, got fixed SHA " + FixedChecksum )]
    [InlineData( "packaging/.SRCINFO", "sha256sums = SKIP", "sha256sums = 'SKIP'",
        "packaging/.SRCINFO sha256sums: expected SKIP template checksum, got 'SKIP'" )]
    [InlineData( "packaging/.SRCINFO", "\tsha256sums = SKIP\n", "",
        "packaging/.SRCINFO sha256sums: expected SKIP template checksum, got missing" )]
    // The flake derives its version and toolchain floor from Cargo.toml.
    [InlineData( "flake.nix", "builtins.readFile ./Cargo.toml", "builtins.readFile ./other.toml",
        "flake.nix package version should be derived from Cargo.toml" )]
    [InlineData( "flake.nix", "versionAtLeast", "versionOlder",
        "flake.nix should compare the selected rustc against Cargo.toml rust-version" )]
    // Install examples that pin a concrete tag are stale one release later.
    [InlineData( "README.md", "wayscriber?ref=RELEASE_TAG", "wayscriber?ref=v{cargo}",
        "README.md: pinned flake ref 'wayscriber?ref=v{cargo}' " + StaleReadme )]
    [InlineData( "README.md", "Open the [latest release](https://github.com/devmobasa/wayscriber/releases/latest)",
        "Open the [release](https://github.com/devmobasa/wayscriber/releases/tag/v{cargo})",
        "README.md: pinned release URL '/releases/tag/v{cargo}' " + StaleReadme )]
    [InlineData( "README.md", "wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/latest/download/",
        "wget -O wayscriber-amd64.deb https://github.com/devmobasa/wayscriber/releases/download/{cargo}/",
        "README.md: pinned release URL '/releases/download/{cargo}' " + StaleReadme )]
    public async Task MetadataDriftIsRejected( string relativePath, string current, string replacement, string expected )
    {
        using var fixture = new VersionMetadataFixture( );
        string WithCargo( string value ) => value.Replace( CargoPlaceholder, fixture.CargoVersion, StringComparison.Ordinal );
        fixture.Replace( relativePath, WithCargo( current ), WithCargo( replacement ) );

        var error = await fixture.CheckExpectingFailure( );

        Assert.Contains( WithCargo( expected ), error, StringComparison.Ordinal );
    }

    [Fact]
    public async Task TableFormLibadwaitaDependencyIsReadStructurally( )
    {
        using var fixture = new VersionMetadataFixture( );
        fixture.Replace( "configurator/Cargo.toml", """libadwaita = { version = "0.9", features = ["v1_4"] }""", string.Empty );
        var manifest = fixture.Read( "configurator/Cargo.toml" );
        var tableForm = "\n[dependencies.libadwaita]\nversion = \"0.9\"\nfeatures = [\n    \"v1_4\", # floor\n]\n";
        fixture.Write( "configurator/Cargo.toml", manifest + tableForm );

        Assert.StartsWith( "Version consistency OK:", await fixture.Check( ), StringComparison.Ordinal );

        fixture.Replace( "configurator/Cargo.toml", "\"v1_4\", # floor", "\"v1_5\"" );

        Assert.Contains( "configurator/Cargo.toml libadwaita features: expected ['v1_4'], got ['v1_5']",
            await fixture.CheckExpectingFailure( ), StringComparison.Ordinal );
    }

    [Fact]
    public async Task LibadwaitaOutsideRuntimeDependenciesIsRejected( )
    {
        using var fixture = new VersionMetadataFixture( );
        const string dependency = """libadwaita = { version = "0.9", features = ["v1_4"] }""";
        fixture.Replace( "configurator/Cargo.toml", dependency + "\n", string.Empty );
        fixture.Replace( "configurator/Cargo.toml", "[dev-dependencies]\n", "[dev-dependencies]\n" + dependency + "\n" );

        var error = await fixture.CheckExpectingFailure( );

        Assert.Contains( "configurator/Cargo.toml: missing structured libadwaita dependency", error, StringComparison.Ordinal );
    }

    [Fact]
    public async Task MissingGlobalJsonIsInvalidSdkMetadata( )
    {
        using var fixture = new VersionMetadataFixture( );
        File.Delete( fixture.PathFor( "global.json" ) );

        var error = await fixture.CheckExpectingFailure( );

        Assert.Contains( "global.json SDK metadata is invalid", error, StringComparison.Ordinal );
        Assert.Contains( "global.json SDK: expected 11.0.100-rc.1.26425.128, got missing", error, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( "1.2", "release version has unsupported format: 1.2" )]
    [InlineData( "v{cargo}", "release version has unsupported format: v{cargo}" )]
    [InlineData( "{cargo}.1.2", "release version has unsupported format: {cargo}.1.2" )]
    [InlineData( "0{cargo}", "release version has unsupported format: 0{cargo}" )]
    [InlineData( "99.98.97", "release version 99.98.97 must equal Cargo version {cargo} or be a hotfix of it, such as {cargo}.1" )]
    [InlineData( "{cargo}0", "release version {cargo}0 must equal Cargo version {cargo} or be a hotfix of it, such as {cargo}.1" )]
    [InlineData( "{cargo}.1", "packaging/PKGBUILD pkgver: expected {cargo}.1, got {cargo}" )]
    [InlineData( "{cargo}.1", "packaging/.SRCINFO pkgver: expected {cargo}.1, got {cargo}" )]
    public async Task ReleaseVersionMustMatchCargoAndThePackagingRecipe( string release, string expected )
    {
        using var fixture = new VersionMetadataFixture( );
        string WithCargo( string value ) => value.Replace( CargoPlaceholder, fixture.CargoVersion, StringComparison.Ordinal );

        var error = await fixture.CheckExpectingFailure( "--release-version", WithCargo( release ) );

        Assert.Contains( WithCargo( expected ), error, StringComparison.Ordinal );
    }

    [Fact]
    public async Task PackagingHotfixIsAcceptedOnlyAsTheNamedRelease( )
    {
        using var fixture = new VersionMetadataFixture( );
        var cargo = fixture.CargoVersion;
        fixture.SetPackagingVersion( cargo + ".1" );

        Assert.StartsWith( $"Version consistency OK: Cargo={cargo}, packaging={cargo}.1,",
            await fixture.Check( ), StringComparison.Ordinal );
        Assert.StartsWith( $"Version consistency OK: Cargo={cargo}, packaging={cargo}.1,",
            await fixture.Check( "--release-version", cargo + ".1" ), StringComparison.Ordinal );
        Assert.Contains( $"packaging/PKGBUILD pkgver: expected {cargo}, got {cargo}.1",
            await fixture.CheckExpectingFailure( "--release-version", cargo ), StringComparison.Ordinal );
        Assert.Contains( $"packaging/.SRCINFO pkgver: expected {cargo}.2, got {cargo}.1",
            await fixture.CheckExpectingFailure( "--release-version", cargo + ".2" ), StringComparison.Ordinal );
    }

    [Fact]
    public async Task HotfixOfAnotherCargoVersionIsRejected( )
    {
        using var fixture = new VersionMetadataFixture( );
        fixture.SetPackagingVersion( "0.0.1.1" );

        var error = await fixture.CheckExpectingFailure( );

        Assert.Contains( $"packaging/PKGBUILD pkgver: expected {fixture.CargoVersion}, got 0.0.1.1", error, StringComparison.Ordinal );
        Assert.Contains( $"packaging/.SRCINFO pkgver: expected {fixture.CargoVersion}, got 0.0.1.1", error, StringComparison.Ordinal );
    }

    // Updating every metadata floor is still incomplete until the package runner
    // moves to a reviewed release-platform contract that supports the new ABI.
    [Fact]
    public void RaisingEveryFloorStillRequiresAReviewedRunnerContract( )
    {
        using var fixture = new VersionMetadataFixture( );
        fixture.Replace( "configurator/Cargo.toml", """features = ["v1_4"]""", """features = ["v1_7"]""" );
        foreach ( var path in new[] { "packaging/PKGBUILD", "packaging/.SRCINFO", "packaging/package.configurator.yaml", ShellAurUpdater } )
        {
            fixture.Replace( path, "1.4", "1.7" );
        }

        var errors = VersionCommands.Validate( fixture.Root, supportedLibadwaitaFloor: "1.7" );

        Assert.Equal( "release package runner ubuntu-24.04 libadwaita floor: expected 1.4, got 1.7", Assert.Single( errors ) );
    }
}
