using Cardfile.Shared.Services;
using Cardfile.Web.Components;
using Cardfile.Web.Services;
using Microsoft.FluentUI.AspNetCore.Components;
using Cardfile.Shared.Data;
using Microsoft.EntityFrameworkCore;
using Microsoft.AspNetCore.Components.Authorization;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.AspNetCore.Authentication;
using System.Security.Claims;

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

// Add HttpContextAccessor for authentication services
builder.Services.AddHttpContextAccessor();

// Add application services
builder.Services.AddScoped<ICardService, CardService>();
builder.Services.AddScoped<ITagService, TagService>();
builder.Services.AddScoped<IUserService, UserService>();
builder.Services.AddScoped<IAppConfigService, AppConfigService>();
builder.Services.AddScoped<ICardAttachmentService, CardAttachmentService>();
// Register authentication services
builder.Services.AddScoped<IAuthService, AuthService>();
builder.Services.AddScoped<AuthenticationStateProvider, CustomAuthenticationStateProvider>();

// Add ASP.NET Core Authentication services with Cookies as default scheme
builder.Services.AddAuthentication(CookieAuthenticationDefaults.AuthenticationScheme)
    .AddCookie(options =>
    {
        options.LoginPath = "/login";
        options.LogoutPath = "/logout";
        options.AccessDeniedPath = "/login";
    });

// Authorization services for Blazor components
builder.Services.AddAuthorizationCore();

// Settings service based on appsettings.json persistence
builder.Services.AddSingleton<IAppSettingsService, AppSettingsService>();

var app = builder.Build();

// Ensure database is created
using (var scope = app.Services.CreateScope())
{
    var db = scope.ServiceProvider.GetRequiredService<CardfileDbContext>();
    await db.EnsureDatabaseCreatedAsync();
}

// Configure the HTTP request pipeline.
if (!app.Environment.IsDevelopment())
{
    app.UseExceptionHandler("/Error", createScopeForErrors: true);
    // The default HSTS value is 30 days. You may want to change this for production scenarios, see https://aka.ms/aspnetcore-hsts.
    app.UseHsts();
}

app.UseHttpsRedirection();

// Add authentication and authorization middleware
app.UseAuthentication();
app.UseAuthorization();

app.UseAntiforgery();

// Minimal API endpoints to issue/clear auth cookies safely via HTTP

// Handle login and issue authentication cookie
app.MapPost("/auth/login", async (HttpContext httpContext, IUserService userService, IAppSettingsService appSettingsService) =>
{
    // Read form values (from standard HTML form post)
    var form = await httpContext.Request.ReadFormAsync();
    var username = form["username"].ToString();
    var password = form["password"].ToString();

    if (string.IsNullOrWhiteSpace(username) || string.IsNullOrWhiteSpace(password))
    {
        return Results.Redirect("/login?error=missing");
    }

    var user = await userService.GetByUsernameAsync(username);
    if (user is null || !user.IsActive || !BCrypt.Net.BCrypt.Verify(password, user.PasswordHash))
    {
        return Results.Redirect("/login?error=invalid");
    }

    // Preserve RememberCredentials preference from stored settings
    var existingLastUser = await appSettingsService.GetLastUserAsync();
    var rememberPreference = existingLastUser?.RememberCredentials ?? false;

    await appSettingsService.UpdateLastUserAsync(new LastUserInfo
    {
        Username = user.Username,
        Email = user.Email ?? string.Empty,
        RememberCredentials = rememberPreference,
        LastLogin = DateTime.UtcNow
    });

    // Build claims and sign-in with cookie
    var claims = new List<Claim>
    {
        new Claim(ClaimTypes.NameIdentifier, user.Id.ToString()),
        new Claim(ClaimTypes.Name, user.Username),
        new Claim(ClaimTypes.Email, user.Email ?? string.Empty)
    };

    var identity = new ClaimsIdentity(claims, CookieAuthenticationDefaults.AuthenticationScheme);
    var principal = new ClaimsPrincipal(identity);

    var authProperties = new AuthenticationProperties
    {
        IsPersistent = rememberPreference,
        ExpiresUtc = rememberPreference ? DateTimeOffset.UtcNow.AddDays(7) : DateTimeOffset.UtcNow.AddHours(8)
    };

    await httpContext.SignInAsync(CookieAuthenticationDefaults.AuthenticationScheme, principal, authProperties);

    // Redirect to main page after successful login
    return Results.Redirect("/cards");
}).DisableAntiforgery();

// Handle logout and clear authentication cookie
app.MapGet("/logout", async (HttpContext httpContext, IAppSettingsService appSettingsService) =>
{
    // Optionally preserve RememberCredentials and clear last user fields
    var existingLastUser = await appSettingsService.GetLastUserAsync();
    var rememberPreference = existingLastUser?.RememberCredentials ?? false;

    await appSettingsService.UpdateLastUserAsync(new LastUserInfo
    {
        Username = string.Empty,
        Email = string.Empty,
        RememberCredentials = rememberPreference,
        LastLogin = DateTime.UtcNow
    });

    await httpContext.SignOutAsync(CookieAuthenticationDefaults.AuthenticationScheme);

    return Results.Redirect("/login");
});

// Configure static files from Cardfile.Shared wwwroot
app.UseStaticFiles();
app.MapStaticAssets();
app.MapRazorComponents<App>()
    .AddInteractiveServerRenderMode()
    .AddAdditionalAssemblies(typeof(Cardfile.Shared._Imports).Assembly);

app.Run();
