package service_a

import (
	"github.com/example/rcm-zone-of-pain/common"
)

// ServiceA depends on common.
type ServiceA struct {
	util common.SharedUtil
}
