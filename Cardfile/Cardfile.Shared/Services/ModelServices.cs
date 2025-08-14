using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services
{
    // Servicio para operaciones con Card
    public class CardService : ICardService
    {
        public Task<IEnumerable<Card>> GetAllAsync() => throw new NotImplementedException();
        public Task<Card?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(Card entity) => throw new NotImplementedException();
        public Task UpdateAsync(Card entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
    }

    // Servicio para operaciones con Tag
    public class TagService : ITagService
    {
        public Task<IEnumerable<Tag>> GetAllAsync() => throw new NotImplementedException();
        public Task<Tag?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(Tag entity) => throw new NotImplementedException();
        public Task UpdateAsync(Tag entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
    }

    // Servicio para operaciones con User
    public class UserService : IUserService
    {
        public Task<IEnumerable<User>> GetAllAsync() => throw new NotImplementedException();
        public Task<User?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(User entity) => throw new NotImplementedException();
        public Task UpdateAsync(User entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
        public Task<User?> GetByUsernameAsync(string username) => throw new NotImplementedException();
        public Task<bool> ValidatePasswordAsync(string username, string password) => throw new NotImplementedException();
    }

    // Servicio para operaciones con AppConfig
    public class AppConfigService : IAppConfigService
    {
        public Task<IEnumerable<AppConfig>> GetAllAsync() => throw new NotImplementedException();
        public Task<AppConfig?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(AppConfig entity) => throw new NotImplementedException();
        public Task UpdateAsync(AppConfig entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
        public Task<AppConfig?> GetCurrentConfigAsync() => throw new NotImplementedException();
    }
}
