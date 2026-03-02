package ports

// UserRepository defines the contract for user persistence.
type UserRepository interface {
	FindByID(id string) error
	Save(name string) error
}

// AuditLogger defines the contract for audit logging.
// This port has no adapter implementing it.
type AuditLogger interface {
	Log(event string) error
}
