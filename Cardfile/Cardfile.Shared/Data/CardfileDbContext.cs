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
        public DbSet<CardAttachment> CardAttachments { get; set; }

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

                // Relación con CardAttachment
                entity.HasMany(c => c.Attachments)
                      .WithOne(a => a.Card)
                      .HasForeignKey(a => a.CardId)
                      .OnDelete(DeleteBehavior.Cascade);
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

            // Configuración de la entidad CardAttachment
            modelBuilder.Entity<CardAttachment>(entity =>
            {
                entity.HasKey(a => a.Id);
                entity.Property(a => a.FileName).IsRequired().HasMaxLength(255);
                entity.Property(a => a.ContentType).IsRequired().HasMaxLength(200);
                entity.Property(a => a.FileSize).IsRequired();
                entity.Property(a => a.FileData).IsRequired();
                entity.Property(a => a.UploadedAt).IsRequired();

                entity.HasIndex(a => a.CardId);
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
    }
}