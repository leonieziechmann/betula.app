package provider

import (
	"context"
	"testing"
)

type mockProvider struct {
	name string
	desc string
}

func (m *mockProvider) Name() string        { return m.name }
func (m *mockProvider) Description() string { return m.desc }

func TestRegistry(t *testing.T) {
	reg := NewRegistry()

	p1 := &mockProvider{name: "prov1", desc: "Provider 1"}
	p2 := &mockProvider{name: "prov2", desc: "Provider 2"}

	if err := reg.Register(p1); err != nil {
		t.Fatalf("failed to register p1: %v", err)
	}
	if err := reg.Register(p2); err != nil {
		t.Fatalf("failed to register p2: %v", err)
	}

	// Duplicate registration error
	if err := reg.Register(p1); err == nil {
		t.Fatalf("expected error on duplicate register, got nil")
	}

	// Get
	retrieved, ok := reg.Get("prov1")
	if !ok || retrieved.Name() != "prov1" {
		t.Errorf("failed to retrieve prov1: ok=%v", ok)
	}

	// List
	list := reg.List()
	if len(list) != 2 {
		t.Errorf("expected 2 providers in list, got %d", len(list))
	}
}

func TestProvidersRegistered(t *testing.T) {
	reg := NewRegistry()
	cat := NewBTUModuleCatalogProvider(nil, nil, "", 0)
	detail := NewBTUModuleDetailProvider(nil, nil, "", 0)

	_ = reg.Register(cat)
	_ = reg.Register(detail)
	event := NewBTUEventProvider(nil, nil, 0)
	_ = reg.Register(event)
	fues := NewBTUFUESProvider(nil, nil, "", 0)
	_ = reg.Register(fues)
	programs := NewBTUProgramTreeProvider(nil, nil, "", 0, "")
	_ = reg.Register(programs)

	if _, ok := reg.Get(CatalogProviderName); !ok {
		t.Errorf("expected %s to be registered", CatalogProviderName)
	}
	if _, ok := reg.Get(DetailProviderName); !ok {
		t.Errorf("expected %s to be registered", DetailProviderName)
	}
	if _, ok := reg.Get(EventProviderName); !ok {
		t.Errorf("expected %s to be registered", EventProviderName)
	}
	if _, ok := reg.Get(FUESProviderName); !ok {
		t.Errorf("expected %s to be registered", FUESProviderName)
	}
	if _, ok := reg.Get(ProgramTreeProviderName); !ok {
		t.Errorf("expected %s to be registered", ProgramTreeProviderName)
	}
}

type dummyCatalogProvider struct {
	mockProvider
}

func (d *dummyCatalogProvider) ScrapeCatalog(ctx context.Context, forceRefresh bool) (int, error) {
	return 42, nil
}

func TestCatalogProviderInterface(t *testing.T) {
	var _ CatalogProvider = &dummyCatalogProvider{}
}
