using Xunit;

namespace Wayscriber.Tools.Tests;

// The Nix CI commands, run against a recording process runner so each
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
