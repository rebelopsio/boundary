package service_b

import (
	"github.com/example/rcm-zone-of-pain/common"
)

// ServiceB depends on common.
type ServiceB struct {
	util common.SharedUtil
}
