namespace Wayscriber.Tools;

// `ci minimum-rust`: checks the workspace with the oldest Rust the manifests declare.
// Inside the checkout `rust-toolchain.toml` selects the newer development toolchain,
// so every Cargo call names the minimum explicitly.
internal static class MinimumRustCommand
{
    private const string MinimalProfile = "minimal";

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new(CommandAreas.ContinuousIntegration, CommandNames.MinimumRust,
            "Check the workspace with the minimum Rust version from Cargo.toml.", SideEffect.MachineMutating, MinimumRust),
    ];

    private static async Task<int> MinimumRust( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "ci minimum-rust" );
        var minimum = VersionCommands.ReadCargoRustVersion( context.Path( RepositoryPaths.CargoManifest ) );
        var configuratorMinimum = VersionCommands.ReadCargoRustVersion( context.Path( RepositoryPaths.ConfiguratorCargoManifest ) );
        if ( configuratorMinimum != minimum )
        {
            throw new ToolException( $"{RepositoryPaths.ConfiguratorCargoManifest} rust-version {configuratorMinimum} " +
                                     $"differs from {RepositoryPaths.CargoManifest} rust-version {minimum}" );
        }

        var toolchain = $"+{minimum}";
        await context.Run( Programs.Rustup, ["toolchain", "install", minimum, "--profile", MinimalProfile, "--no-self-update"] );
        await context.Run( Programs.Cargo, [toolchain, CommandLineOptions.Version] );

        foreach ( var features in new[] { CommandLineOptions.AllFeatures, CommandLineOptions.NoDefaultFeatures } )
        {
            await context.Run( Programs.Cargo,
                [toolchain, "check", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.AllTargets, features] );
        }

        return ExitCodes.Success;
    }
}
