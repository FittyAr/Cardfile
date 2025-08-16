using Microsoft.EntityFrameworkCore;
using Cardfile.Shared.Models;

namespace Cardfile.Shared.Data
{
    /// <summary>
    /// Contexto de base de datos para Cardfile
    /// </summary>
    public class CardfileDbContext : DbContext
    {
        public CardfileDbContext(DbContextOptions<CardfileDbContext> options) : base(options)
        {
        }

        // DbSets para todas las entidades
        public DbSet<Card> Cards { get; set; }
        public DbSet<Tag> Tags { get; set; }
        public DbSet<CardTag> CardTags { get; set; }
        public DbSet<User> Users { get; set; }
        public DbSet<AppConfig> AppConfigs { get; set; }

        protected override void OnModelCreating(ModelBuilder modelBuilder)
        {
            base.OnModelCreating(modelBuilder);

            // Configuración de la entidad Card
            modelBuilder.Entity<Card>(entity =>
            {
                entity.HasKey(c => c.Id);
                entity.Property(c => c.Title).IsRequired().HasMaxLength(200);
                entity.Property(c => c.Content).HasMaxLength(2000);
                entity.Property(c => c.CreatedAt).IsRequired();
                entity.Property(c => c.UpdatedAt);

                // Relación con User
                entity.HasOne(c => c.User)
                      .WithMany(u => u.Cards)
                      .HasForeignKey(c => c.UserId)
                      .OnDelete(DeleteBehavior.Cascade);

                // Índices para mejorar el rendimiento
                entity.HasIndex(c => c.CreatedAt);
                entity.HasIndex(c => c.UpdatedAt);
                entity.HasIndex(c => c.UserId);
            });

            // Configuración de la entidad Tag
            modelBuilder.Entity<Tag>(entity =>
            {
                entity.HasKey(t => t.Id);
                entity.Property(t => t.Name).IsRequired().HasMaxLength(100);

                // Índice único para el nombre del tag
                entity.HasIndex(t => t.Name).IsUnique();
            });

            // Configuración de la entidad CardTag (muchos a muchos)
            modelBuilder.Entity<CardTag>(entity =>
            {
                entity.HasKey(ct => new { ct.CardId, ct.TagId });

                entity.HasOne(ct => ct.Card)
                      .WithMany(c => c.CardTags)
                      .HasForeignKey(ct => ct.CardId)
                      .OnDelete(DeleteBehavior.Cascade);

                entity.HasOne(ct => ct.Tag)
                      .WithMany(t => t.CardTags)
                      .HasForeignKey(ct => ct.TagId)
                      .OnDelete(DeleteBehavior.Cascade);
            });

            // Configuración de la entidad User
            modelBuilder.Entity<User>(entity =>
            {
                entity.HasKey(u => u.Id);
                entity.Property(u => u.Username).IsRequired().HasMaxLength(100);
                entity.Property(u => u.Email).HasMaxLength(200);
                entity.Property(u => u.PasswordHash).IsRequired().HasMaxLength(500);
                entity.Property(u => u.IsActive).IsRequired();
                entity.Property(u => u.CreatedAt).IsRequired();

                // Índice único para el nombre de usuario
                entity.HasIndex(u => u.Username).IsUnique();
                
                // Índice para email si existe
                entity.HasIndex(u => u.Email).IsUnique();
            });

            // Configuración de la entidad AppConfig
            modelBuilder.Entity<AppConfig>(entity =>
            {
                entity.HasKey(ac => ac.Id);
                entity.Property(ac => ac.DatabaseType).IsRequired().HasMaxLength(50);
                entity.Property(ac => ac.ConnectionString).HasMaxLength(1000);
                entity.Property(ac => ac.Language).HasMaxLength(20);
                entity.Property(ac => ac.LastUser).HasMaxLength(100);
                entity.Property(ac => ac.LastUserEmail).HasMaxLength(200);
            });
        }

        /// <summary>
        /// Método para aplicar migraciones automáticamente
        /// </summary>
        public async Task EnsureDatabaseCreatedAsync()
        {
            await Database.EnsureCreatedAsync();
        }

        /// <summary>
        /// Método para inicializar datos de prueba
        /// </summary>
        public async Task SeedDataAsync()
        {
            if (!Users.Any())
            {
                // Crear usuario administrador por defecto
                var adminUser = new User
                {
                    Id = Guid.NewGuid(),
                    Username = "admin",
                    Email = "admin@cardfile.com",
                    PasswordHash = BCrypt.Net.BCrypt.HashPassword("admin123"), // Usaremos BCrypt para hash
                    IsActive = true,
                    CreatedAt = DateTime.UtcNow
                };

                Users.Add(adminUser);

                // Crear algunos tags por defecto
                var workTag = new Tag { Id = Guid.NewGuid(), Name = "trabajo" };
                var personalTag = new Tag { Id = Guid.NewGuid(), Name = "personal" };
                var ideasTag = new Tag { Id = Guid.NewGuid(), Name = "ideas" };

                Tags.AddRange(workTag, personalTag, ideasTag);

                // Crear algunas tarjetas de ejemplo
                var card1 = new Card
                {
                    Id = Guid.NewGuid(),
                    Title = "Bienvenido a Cardfile",
                    Content = "Esta es tu primera tarjeta. Puedes usarla para organizar tus pensamientos, ideas e información importante.",
                    CreatedAt = DateTime.UtcNow.AddDays(-5),
                    UpdatedAt = DateTime.UtcNow.AddDays(-2),
                    UserId = adminUser.Id
                };

                var card2 = new Card
                {
                    Id = Guid.NewGuid(),
                    Title = "Ideas de Proyectos",
                    Content = "Colección de ideas de proyectos para desarrollo futuro. Incluye aplicaciones web, apps móviles y herramientas de escritorio.",
                    CreatedAt = DateTime.UtcNow.AddDays(-3),
                    UserId = adminUser.Id
                };

                var card3 = new Card
                {
                    Id = Guid.NewGuid(),
                    Title = "Recursos de Aprendizaje",
                    Content = "Recursos de aprendizaje importantes y tutoriales para desarrollo de habilidades.",
                    CreatedAt = DateTime.UtcNow.AddDays(-1),
                    UserId = adminUser.Id
                };

                Cards.AddRange(card1, card2, card3);

                // Crear relaciones CardTag
                CardTags.AddRange(
                    new CardTag { CardId = card1.Id, TagId = personalTag.Id },
                    new CardTag { CardId = card2.Id, TagId = workTag.Id },
                    new CardTag { CardId = card2.Id, TagId = ideasTag.Id },
                    new CardTag { CardId = card3.Id, TagId = workTag.Id }
                );

                await SaveChangesAsync();
            }
        }
    }
}