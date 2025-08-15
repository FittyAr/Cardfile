using Cardfile.Services;
using Cardfile.Shared.Services;
using Microsoft.Extensions.Logging;
using Microsoft.FluentUI.AspNetCore.Components;

namespace Cardfile
{
    public static class MauiProgram
    {
        public static MauiApp CreateMauiApp()
        {
            var builder = MauiApp.CreateBuilder();
            builder
                .UseMauiApp<App>()
                .ConfigureFonts(fonts =>
                {
                    fonts.AddFont("OpenSans-Regular.ttf", "OpenSansRegular");
                });

            // Add device-specific services used by the Cardfile.Shared project
            builder.Services.AddSingleton<IFormFactor, FormFactor>();

            // Add application services
            builder.Services.AddSingleton<IAppConfigService, AppConfigService>();
            builder.Services.AddSingleton<IAppSettingsService, AppSettingsService>();

            builder.Services.AddMauiBlazorWebView();
            builder.Services.AddFluentUIComponents();

#if DEBUG
            builder.Services.AddBlazorWebViewDeveloperTools();
            builder.Logging.AddDebug();
#endif

            return builder.Build();
        }
    }
}
