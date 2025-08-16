using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Cardfile.Shared.Models;
using Cardfile.Shared.Data;
using Microsoft.EntityFrameworkCore;

namespace Cardfile.Shared.Services
{
    // Servicio para operaciones con Card
    public class CardService : ICardService
    {
        private readonly CardfileDbContext _db;

        public CardService(CardfileDbContext db)
        {
            _db = db;
        }

        public async Task<IEnumerable<Card>> GetAllAsync()
        {
            return await _db.Cards
                .Include(c => c.CardTags)
                    .ThenInclude(ct => ct.Tag)
                .AsNoTracking()
                .OrderByDescending(c => c.CreatedAt)
                .ToListAsync();
        }

        public async Task<Card?> GetByIdAsync(Guid id)
        {
            return await _db.Cards
                .Include(c => c.CardTags)
                    .ThenInclude(ct => ct.Tag)
                .FirstOrDefaultAsync(c => c.Id == id);
        }

        public async Task AddAsync(Card entity)
        {
            if (entity.Id == Guid.Empty)
                entity.Id = Guid.NewGuid();
            if (entity.CreatedAt == default)
                entity.CreatedAt = DateTime.UtcNow;

            // Ensure the Card is linked to a valid User to satisfy FK constraint
            if (entity.UserId == Guid.Empty)
            {
                // Try to use the first available user (seeded admin) as a sensible default
                var defaultUser = await _db.Users.OrderBy(u => u.CreatedAt).FirstOrDefaultAsync();
                if (defaultUser == null)
                {
                    // As a safety net, create a default admin user if database is empty
                    var adminUser = new User
                    {
                        Id = Guid.NewGuid(),
                        Username = "admin",
                        Email = "admin@cardfile.com",
                        PasswordHash = BCrypt.Net.BCrypt.HashPassword("admin123"),
                        IsActive = true,
                        CreatedAt = DateTime.UtcNow
                    };
                    await _db.Users.AddAsync(adminUser);
                    await _db.SaveChangesAsync();
                    defaultUser = adminUser;
                }
                entity.UserId = defaultUser.Id;
            }

            // Asegurar que las relaciones CardTags no creen Tags duplicados
            if (entity.CardTags != null)
            {
                foreach (var ct in entity.CardTags)
                {
                    // Asegurar Tag existente o attach si viene con Id
                    if (ct.Tag != null)
                    {
                        var existingTag = await _db.Tags.FirstOrDefaultAsync(t => t.Name == ct.Tag.Name);
                        if (existingTag != null)
                        {
                            ct.TagId = existingTag.Id;
                            ct.Tag = existingTag;
                        }
                        else
                        {
                            if (ct.Tag.Id == Guid.Empty) ct.Tag.Id = Guid.NewGuid();
                            await _db.Tags.AddAsync(ct.Tag);
                            ct.TagId = ct.Tag.Id;
                        }
                    }
                }
            }

            await _db.Cards.AddAsync(entity);
            await _db.SaveChangesAsync();
        }

        public async Task UpdateAsync(Card entity)
        {
            var existing = await _db.Cards
                .Include(c => c.CardTags)
                .FirstOrDefaultAsync(c => c.Id == entity.Id);

            if (existing == null) return;

            existing.Title = entity.Title;
            existing.Content = entity.Content;
            existing.UpdatedAt = DateTime.UtcNow;

            // Actualizar tags: reemplazo simple
            if (entity.CardTags != null)
            {
                // Eliminar relaciones que ya no están
                var toRemove = existing.CardTags.Where(ct => !entity.CardTags.Any(nct => nct.TagId == ct.TagId || (nct.Tag != null && nct.Tag.Name == ct.Tag!.Name))).ToList();
                _db.CardTags.RemoveRange(toRemove);

                // Agregar nuevas relaciones
                foreach (var nct in entity.CardTags)
                {
                    Guid tagId;
                    if (nct.Tag != null)
                    {
                        var tag = await _db.Tags.FirstOrDefaultAsync(t => t.Name == nct.Tag.Name);
                        if (tag == null)
                        {
                            tag = new Tag { Id = Guid.NewGuid(), Name = nct.Tag.Name };
                            _db.Tags.Add(tag);
                            await _db.SaveChangesAsync();
                        }
                        tagId = tag.Id;
                    }
                    else
                    {
                        tagId = nct.TagId;
                    }

                    if (!existing.CardTags.Any(ct => ct.TagId == tagId))
                    {
                        existing.CardTags.Add(new CardTag { CardId = existing.Id, TagId = tagId });
                    }
                }
            }

            await _db.SaveChangesAsync();
        }

        public async Task DeleteAsync(Guid id)
        {
            var existing = await _db.Cards.FindAsync(id);
            if (existing != null)
            {
                _db.Cards.Remove(existing);
                await _db.SaveChangesAsync();
            }
        }
    }

    // Servicio para operaciones con Tag
    public class TagService : ITagService
    {
        private readonly CardfileDbContext _db;

        public TagService(CardfileDbContext db)
        {
            _db = db;
        }

        public async Task<IEnumerable<Tag>> GetAllAsync()
        {
            return await _db.Tags
                .Include(t => t.CardTags)
                .AsNoTracking()
                .OrderBy(t => t.Name)
                .ToListAsync();
        }

        public Task<Tag?> GetByIdAsync(Guid id) => _db.Tags.FirstOrDefaultAsync(t => t.Id == id);

        public async Task AddAsync(Tag entity)
        {
            if (entity.Id == Guid.Empty) entity.Id = Guid.NewGuid();
            await _db.Tags.AddAsync(entity);
            await _db.SaveChangesAsync();
        }

        public async Task UpdateAsync(Tag entity)
        {
            var existing = await _db.Tags.FindAsync(entity.Id);
            if (existing == null) return;
            existing.Name = entity.Name;
            await _db.SaveChangesAsync();
        }

        public async Task DeleteAsync(Guid id)
        {
            var existing = await _db.Tags.FindAsync(id);
            if (existing != null)
            {
                _db.Tags.Remove(existing);
                await _db.SaveChangesAsync();
            }
        }
    }

    // Servicio para operaciones con User
    public class UserService : IUserService
    {
        private readonly CardfileDbContext _db;

        public UserService(CardfileDbContext db)
        {
            _db = db;
        }

        public async Task<IEnumerable<User>> GetAllAsync() => await _db.Users.AsNoTracking().ToListAsync();
        public Task<User?> GetByIdAsync(Guid id) => _db.Users.FirstOrDefaultAsync(u => u.Id == id);
        public async Task AddAsync(User entity)
        {
            if (entity.Id == Guid.Empty) entity.Id = Guid.NewGuid();
            if (string.IsNullOrWhiteSpace(entity.PasswordHash))
                throw new ArgumentException("PasswordHash requerido");
            await _db.Users.AddAsync(entity);
            await _db.SaveChangesAsync();
        }
        public async Task UpdateAsync(User entity)
        {
            var existing = await _db.Users.FindAsync(entity.Id);
            if (existing == null) return;
            existing.Username = entity.Username;
            existing.Email = entity.Email;
            existing.IsActive = entity.IsActive;
            await _db.SaveChangesAsync();
        }
        public async Task DeleteAsync(Guid id)
        {
            var existing = await _db.Users.FindAsync(id);
            if (existing != null)
            {
                _db.Users.Remove(existing);
                await _db.SaveChangesAsync();
            }
        }
        public Task<User?> GetByUsernameAsync(string username) => _db.Users.FirstOrDefaultAsync(u => u.Username == username);
        public async Task<bool> ValidatePasswordAsync(string username, string password)
        {
            var user = await _db.Users.FirstOrDefaultAsync(u => u.Username == username);
            if (user == null) return false;
            return BCrypt.Net.BCrypt.Verify(password, user.PasswordHash);
        }
    }

    // Servicio para operaciones con AppConfig
    public class AppConfigService : IAppConfigService
    {
        private readonly CardfileDbContext _db;

        public AppConfigService(CardfileDbContext db)
        {
            _db = db;
        }

        public async Task<IEnumerable<AppConfig>> GetAllAsync() => await _db.AppConfigs.AsNoTracking().ToListAsync();
        public Task<AppConfig?> GetByIdAsync(Guid id) => _db.AppConfigs.FirstOrDefaultAsync(c => c.Id == id);
        public async Task AddAsync(AppConfig entity)
        {
            if (entity.Id == Guid.Empty) entity.Id = Guid.NewGuid();
            await _db.AppConfigs.AddAsync(entity);
            await _db.SaveChangesAsync();
        }
        public async Task UpdateAsync(AppConfig entity)
        {
            var existing = await _db.AppConfigs.FindAsync(entity.Id);
            if (existing == null) return;
            existing.DatabaseType = entity.DatabaseType;
            existing.ConnectionString = entity.ConnectionString;
            existing.Language = entity.Language;
            existing.RememberUser = entity.RememberUser;
            existing.RememberPassword = entity.RememberPassword;
            existing.LastUser = entity.LastUser;
            existing.LastUserEmail = entity.LastUserEmail;
            existing.LastLoginDate = entity.LastLoginDate;
            existing.RememberCredentials = entity.RememberCredentials;
            await _db.SaveChangesAsync();
        }
        public async Task DeleteAsync(Guid id)
        {
            var existing = await _db.AppConfigs.FindAsync(id);
            if (existing != null)
            {
                _db.AppConfigs.Remove(existing);
                await _db.SaveChangesAsync();
            }
        }

        public async Task<AppConfig?> GetCurrentConfigAsync()
        {
            return await _db.AppConfigs.OrderByDescending(c => c.LastLoginDate).FirstOrDefaultAsync();
        }

        public async Task SetCurrentConfigAsync(AppConfig config)
        {
            if (config.Id == Guid.Empty)
            {
                await AddAsync(config);
            }
            else
            {
                await UpdateAsync(config);
            }
        }

        public async Task<bool> IsConfiguredAsync()
        {
            return await _db.AppConfigs.AnyAsync();
        }
    }
}
