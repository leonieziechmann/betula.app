package provider

import (
	"context"
	"fmt"
	"sync"

	"github.com/jakob/btu-scraper/internal/model"
)

// Provider represents a generic data source or scraper provider.
type Provider interface {
	Name() string
	Description() string
}

// Registry manages registered information providers.
type Registry struct {
	mu        sync.RWMutex
	providers map[string]Provider
}

// NewRegistry initializes an empty provider registry.
func NewRegistry() *Registry {
	return &Registry{
		providers: make(map[string]Provider),
	}
}

// Register adds a provider to the registry.
func (r *Registry) Register(p Provider) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	name := p.Name()
	if _, exists := r.providers[name]; exists {
		return fmt.Errorf("provider %q is already registered", name)
	}
	r.providers[name] = p
	return nil
}

// Get returns the provider registered under the given name.
func (r *Registry) Get(name string) (Provider, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	p, ok := r.providers[name]
	return p, ok
}

// List returns all registered providers.
func (r *Registry) List() []Provider {
	r.mu.RLock()
	defer r.mu.RUnlock()

	list := make([]Provider, 0, len(r.providers))
	for _, p := range r.providers {
		list = append(list, p)
	}
	return list
}

// CatalogProvider defines the capability to discover modules.
type CatalogProvider interface {
	Provider
	ScrapeCatalog(ctx context.Context, forceRefresh bool) (int, error)
}

// DetailProvider defines the capability to scrape module details.
type DetailProvider interface {
	Provider
	ScrapeModule(ctx context.Context, moduleID string, forceRefresh bool) error
}

// EventProvider defines the capability to scrape course event and schedule details.
type EventProvider interface {
	Provider
	ScrapeEvent(ctx context.Context, eventID, pageURL string, forceRefresh bool) (*model.EventDetail, error)
	ScrapeEventsForModule(ctx context.Context, moduleID string, forceRefresh bool) (int, error)
}
