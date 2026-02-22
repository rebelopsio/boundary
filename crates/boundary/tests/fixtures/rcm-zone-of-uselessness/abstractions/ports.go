package abstractions

import (
	"github.com/example/rcm-zone-of-uselessness/foundation"
)

// AbstractPort is abstract and depends on foundation; nothing depends on it.
type AbstractPort interface {
	Execute(base foundation.Base) error
}
