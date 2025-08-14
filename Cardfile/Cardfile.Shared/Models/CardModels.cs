using System;
using System.Collections.Generic;
using System.ComponentModel.DataAnnotations;

namespace Cardfile.Shared.Models
{
    // Modelo principal para una ficha/tarjeta
    // Este modelo es compatible con diferentes proveedores de base de datos
    public class Card
    {
        // Identificador único de la tarjeta
        [Key]
        public Guid Id { get; set; }

        // Título de la tarjeta
        [Required]
        [MaxLength(200)]
        public string Title { get; set; } = string.Empty;

        // Contenido o descripción de la tarjeta
        [MaxLength(2000)]
        public string? Content { get; set; }

        // Fecha de creación de la tarjeta
        public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

        // Fecha de última modificación
        public DateTime? UpdatedAt { get; set; }

        // Relación con etiquetas (muchos a muchos)
        public ICollection<CardTag> CardTags { get; set; } = new List<CardTag>();
    }

    // Modelo para etiquetas
    public class Tag
    {
        // Identificador único de la etiqueta
        [Key]
        public Guid Id { get; set; }

        // Nombre de la etiqueta
        [Required]
        [MaxLength(100)]
        public string Name { get; set; } = string.Empty;

        // Relación con tarjetas (muchos a muchos)
        public ICollection<CardTag> CardTags { get; set; } = new List<CardTag>();
    }

    // Entidad de unión para la relación muchos a muchos entre Card y Tag
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
        // Identificador único del usuario
        [Key]
        public Guid Id { get; set; }

        // Nombre de usuario
        [Required]
        [MaxLength(100)]
        public string Username { get; set; } = string.Empty;

        // Correo electrónico
        [MaxLength(200)]
        public string? Email { get; set; }

        // Contraseña hasheada
        [Required]
        [MaxLength(500)]
        public string PasswordHash { get; set; } = string.Empty;

        // Indica si el usuario está activo
        public bool IsActive { get; set; } = true;

        // Fecha de creación del usuario
        public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    }

    // Modelo para configuración de la aplicación
    public class AppConfig
    {
        // Identificador único de la configuración
        [Key]
        public Guid Id { get; set; }

        // Tipo de base de datos (ej: SQLite, PostgreSQL)
        [Required]
        [MaxLength(50)]
        public string DatabaseType { get; set; } = "SQLite";

        // Cadena de conexión personalizada (si aplica)
        [MaxLength(1000)]
        public string? ConnectionString { get; set; }

        // Idioma actual de la app
        [MaxLength(20)]
        public string? Language { get; set; }

        // Recordar usuario
        public bool RememberUser { get; set; } = false;

        // Recordar contraseña
        public bool RememberPassword { get; set; } = false;

        // Último usuario recordado
        [MaxLength(100)]
        public string? LastUser { get; set; }
    }
}
