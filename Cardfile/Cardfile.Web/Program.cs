using Cardfile.Shared.Services;
using Cardfile.Web.Components;
using Cardfile.Web.Services;
using Microsoft.FluentUI.AspNetCore.Components;
using Cardfile.Shared.Data;
using Microsoft.EntityFrameworkCore;

var builder = WebApplication.CreateBuilder(args);

// Add services to the container.
builder.Services.AddRazorComponents()
    .AddInteractiveServerComponents();
builder.Services.AddFluentUIComponents();

// Configure EF Core DbContext
var databaseType = builder.Configuration["cardfileSettings:databaseConfiguration:databaseType"] ?? "SQLite";
var connectionString = builder.Configuration["cardfileSettings:databaseConfiguration:connectionString"] ?? "Data Source=cardfile.db";

builder.Services.AddDbContext<CardfileDbContext>(options =>
{
    if (databaseType.Equals("PostgreSQL", StringComparison.OrdinalIgnoreCase) || databaseType.Equals("Postgres", StringComparison.OrdinalIgnoreCase))
    {
        options.UseNpgsql(connectionString);
    }
    else
    {
        options.UseSqlite(connectionString);
    }
});

// Add device-specific services used by the Cardfile.Shared project
builder.Services.AddSingleton<IFormFactor, FormFactor>();

// Add application services
builder.Services.AddScoped<ICardService, CardService>();
builder.Services.AddScoped<ITagService, TagService>();
builder.Services.AddScoped<IUserService, UserService>();
builder.Services.AddScoped<IAppConfigService, AppConfigService>();
builder.Services.AddScoped<ICardAttachmentService, CardAttachmentService>();

// Settings service based on appsettings.json persistence
builder.Services.AddSingleton<IAppSettingsService, AppSettingsService>();

var app = builder.Build();

// Ensure database is created and seeded
using (var scope = app.Services.CreateScope())
{
    var db = scope.ServiceProvider.GetRequiredService<CardfileDbContext>();
    await db.EnsureDatabaseCreatedAsync();
    await db.SeedDataAsync();
}

// Configure the HTTP request pipeline.
if (!app.Environment.IsDevelopment())
{
    app.UseExceptionHandler("/Error", createScopeForErrors: true);
    // The default HSTS value is 30 days. You may want to change this for production scenarios, see https://aka.ms/aspnetcore-hsts.
    app.UseHsts();
}

app.UseHttpsRedirection();


app.UseAntiforgery();

app.MapStaticAssets();
app.MapRazorComponents<App>()
    .AddInteractiveServerRenderMode()
    .AddAdditionalAssemblies(typeof(Cardfile.Shared._Imports).Assembly);

app.Run();
