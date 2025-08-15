using System;
using System.Collections.Generic;
using System.ComponentModel.DataAnnotations;

namespace Cardfile.Shared.Models
{
    // Modelo principal para una ficha/tarjeta
    // Este modelo es compatible con diferentes proveedores de base de datos
    public class Card
    {
        // Identificador �nico de la tarjeta
        [Key]
        public Guid Id { get; set; }

        // T�tulo de la tarjeta
        [Required]
        [MaxLength(200)]
        public string Title { get; set; } = string.Empty;

        // Contenido o descripci�n de la tarjeta
        [MaxLength(2000)]
        public string? Content { get; set; }

        // Fecha de creaci�n de la tarjeta
        public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

        // Fecha de �ltima modificaci�n
        public DateTime? UpdatedAt { get; set; }

        // Relaci�n con etiquetas (muchos a muchos)
        public ICollection<CardTag> CardTags { get; set; } = new List<CardTag>();

        // Relaci�n con el usuario propietario de la tarjeta
        public Guid UserId { get; set; } // Id del usuario propietario
        public User User { get; set; } = null!; // Navegaci�n al usuario
    }

    // Modelo para etiquetas
    public class Tag
    {
        // Identificador �nico de la etiqueta
        [Key]
        public Guid Id { get; set; }

        // Nombre de la etiqueta
        [Required]
        [MaxLength(100)]
        public string Name { get; set; } = string.Empty;

        // Relaci�n con tarjetas (muchos a muchos)
        public ICollection<CardTag> CardTags { get; set; } = new List<CardTag>();
    }

    // Entidad de uni�n para la relaci�n muchos a muchos entre Card y Tag
    public class CardTag
    {
        // Id de la tarjeta
        public Guid CardId { get; set; }
        public Card Card { get; set; } = null!;

        // Id de la etiqueta
        public Guid TagId { get; set; }
        public Tag Tag { get; set; } = null!;
    }

    // Modelo para usuarios del sistema (login)
    public class User
    {
        // Identificador �nico del usuario
        [Key]
        public Guid Id { get; set; }

        // Nombre de usuario
        [Required]
        [MaxLength(100)]
        public string Username { get; set; } = string.Empty;

        // Correo electr�nico
        [MaxLength(200)]
        public string? Email { get; set; }

        // Contrase�a hasheada
        [Required]
        [MaxLength(500)]
        public string PasswordHash { get; set; } = string.Empty;

        // Indica si el usuario est� activo
        public bool IsActive { get; set; } = true;

        // Fecha de creaci�n del usuario
        public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

        // Relaci�n con las tarjetas del usuario
        public ICollection<Card> Cards { get; set; } = new List<Card>();
    }

    // Modelo para configuraci�n de la aplicaci�n
    public class AppConfig
    {
        // Identificador �nico de la configuraci�n
        [Key]
        public Guid Id { get; set; }

        // Tipo de base de datos (ej: SQLite, PostgreSQL)
        [Required]
        [MaxLength(50)]
        public string DatabaseType { get; set; } = "SQLite";

        // Cadena de conexi�n personalizada (si aplica)
        [MaxLength(1000)]
        public string? ConnectionString { get; set; }

        // Idioma actual de la app
        [MaxLength(20)]
        public string? Language { get; set; }

        // Recordar usuario
        public bool RememberUser { get; set; } = false;

        // Recordar contrase�a
        public bool RememberPassword { get; set; } = false;

        // �ltimo usuario recordado
        [MaxLength(100)]
        public string? LastUser { get; set; }

        // Correo del �ltimo usuario
        [MaxLength(200)]
        public string? LastUserEmail { get; set; }

        // Fecha del �ltimo login
        public DateTime? LastLoginDate { get; set; }

        // Indica si se deben recordar las credenciales
        public bool RememberCredentials { get; set; } = false;
    }
}
