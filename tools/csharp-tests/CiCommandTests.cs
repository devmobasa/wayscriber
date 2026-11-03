using Xunit;

namespace Wayscriber.Tools.Tests;

// The Nix and minimum-Rust CI commands, run against a recording process runner so each
// test sees the exact programs and arguments without building anything.
public sealed class CiCommandTests
{
    [Fact]
    public async Task NixBuildBuildsWhatInstantiationOnlyEvaluates( )
    {
        using var fixture = new VersionMetadataFixture( );
        var instantiation = new RecordingRunner( );
        var build = new RecordingRunner( );

        await Command( DevelopmentCommands.Commands, CommandAreas.Check, CommandNames.NixInstantiation )
            .Handler( fixture.Context( instantiation ), [] );
        await Command( DevelopmentCommands.Commands, CommandAreas.Check, CommandNames.NixBuild )
            .Handler( fixture.Context( build ), [] );

        Assert.Equal( [$"{Programs.Nix} build {string.Join( ' ', DevelopmentCommands.NixFlakeTargets )} --no-link --dry-run"],
            instantiation.Commands );
        Assert.Equal( [$"{Programs.Nix} build {string.Join( ' ', DevelopmentCommands.NixFlakeTargets )} --no-link --print-build-logs"],
            build.Commands );
    }

    [Fact]
    public async Task MinimumRustChecksBothFeatureSetsWithTheDeclaredToolchain( )
    {
        using var fixture = new VersionMetadataFixture( );
        var minimum = VersionCommands.ReadCargoRustVersion( fixture.PathFor( "Cargo.toml" ) );
        var runner = new RecordingRunner( );

        var exitCode = await MinimumRust( ).Handler( fixture.Context( runner ), [] );

        Assert.Equal( ExitCodes.Success, exitCode );
        Assert.Equal(
            [
                $"rustup toolchain install {minimum} --profile minimal --no-self-update",
                $"cargo +{minimum} --version",
                $"cargo +{minimum} check --locked --workspace --all-targets --all-features",
                $"cargo +{minimum} check --locked --workspace --all-targets --no-default-features",
            ],
            runner.Commands );
    }

    [Fact]
    public async Task MinimumRustRefusesCratesThatDisagree( )
    {
        using var fixture = new VersionMetadataFixture( );
        var minimum = VersionCommands.ReadCargoRustVersion( fixture.PathFor( "Cargo.toml" ) );
        fixture.Replace( "configurator/Cargo.toml", $"rust-version = \"{minimum}\"", "rust-version = \"1.0.0\"" );
        var runner = new RecordingRunner( );

        var error = await Assert.ThrowsAsync<ToolException>( ( ) => MinimumRust( ).Handler( fixture.Context( runner ), [] ) );

        Assert.Equal( $"configurator/Cargo.toml rust-version 1.0.0 differs from Cargo.toml rust-version {minimum}", error.Message );
        Assert.Empty( runner.Commands );
    }

    private static ToolCommand MinimumRust( ) =>
        Command( MinimumRustCommand.Commands, CommandAreas.ContinuousIntegration, CommandNames.MinimumRust );

    private static ToolCommand Command( IReadOnlyList<ToolCommand> commands, string area, string name ) =>
        commands.Single( command => command.Area == area && command.Name == name );

    private sealed class RecordingRunner : IProcessRunner
    {
        public List<string> Commands { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Commands.Add( string.Join( ' ', [request.FileName, .. request.Arguments] ) );
            return Task.FromResult( new ProcessResult( ExitCodes.Success, string.Empty, string.Empty ) );
        }
    }
}
