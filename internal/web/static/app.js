// BTU Smart Modulkatalog - LocalStorage Synchronizer & Interactive UI Logic

const STORAGE_KEYS = {
  PROGRAM: 'btu_selected_program',
  PROGRAM_TITLE: 'btu_selected_program_title',
  TURNUS: 'btu_target_semester',
  COMPLETED: 'btu_completed_modules',
  BOOKMARKS: 'btu_bookmarked_modules',
  ONLY_FUES: 'btu_only_fues',
  HIDE_PHASE_OUT: 'btu_hide_phase_out',
  PREREQS_MET: 'btu_prereqs_met',
  VIEW_MODE: 'btu_view_mode',
  LANGUAGE: 'btu_language',
  MIN_CREDITS: 'btu_min_credits'
};

let activeSpecialView = null; // null | 'bookmarks' | 'completed'

// State helpers
function getCompletedModules() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEYS.COMPLETED) || '[]');
  } catch(e) {
    return [];
  }
}

function saveCompletedModules(list) {
  localStorage.setItem(STORAGE_KEYS.COMPLETED, JSON.stringify(list));
  updateBadges();
}

function getBookmarkedModules() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEYS.BOOKMARKS) || '[]');
  } catch(e) {
    return [];
  }
}

function saveBookmarkedModules(list) {
  localStorage.setItem(STORAGE_KEYS.BOOKMARKS, JSON.stringify(list));
  updateBadges();
}

// Toggle functions
window.toggleCompleted = function(moduleId, event) {
  if (event) event.stopPropagation();
  let list = getCompletedModules();
  const index = list.indexOf(moduleId);
  if (index > -1) {
    list.splice(index, 1);
  } else {
    list.push(moduleId);
  }
  saveCompletedModules(list);
  refreshModules();
};

window.toggleBookmark = function(moduleId, event) {
  if (event) event.stopPropagation();
  let list = getBookmarkedModules();
  const index = list.indexOf(moduleId);
  if (index > -1) {
    list.splice(index, 1);
  } else {
    list.push(moduleId);
  }
  saveBookmarkedModules(list);
  refreshModules();
};

// Anonymous GDPR-compliant telemetry helper (discards all personal info)
function trackAnonymousEvent(type, targetId, targetName) {
  try {
    const payload = JSON.stringify({
      type: type,
      target_id: targetId || '',
      target_name: targetName || ''
    });
    if (navigator.sendBeacon) {
      navigator.sendBeacon('/api/track', new Blob([payload], { type: 'application/json' }));
    } else {
      fetch('/api/track', {
        method: 'POST',
        body: payload,
        headers: { 'Content-Type': 'application/json' },
        keepalive: true
      }).catch(() => {});
    }
  } catch (e) {
    // Graceful error ignore
  }
}

// Modal functions with background scroll locking
window.openModal = function(moduleId) {
  document.body.classList.add('modal-open');
  trackAnonymousEvent('module_click', moduleId);
  htmx.ajax('GET', `/modules/${moduleId}`, '#modal-container');
};

window.closeModal = function() {
  document.body.classList.remove('modal-open');
  const container = document.getElementById('modal-container');
  if (container) container.innerHTML = '';
};

// Calendar Block -> Accordion linking
window.selectCalendarEvent = function(eventId) {
  const item = document.getElementById(`accordion-${eventId}`);
  if (item) {
    item.classList.add('open');
    item.classList.add('highlighted');
    item.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    setTimeout(() => item.classList.remove('highlighted'), 2000);
  }
};

window.toggleAccordion = function(eventId) {
  const item = document.getElementById(`accordion-${eventId}`);
  if (item) {
    item.classList.toggle('open');
  }
};

// Responsive Sidebar Drawer Logic
window.toggleSidebar = function(forceOpen) {
  const sidebar = document.getElementById('sidebar');
  const backdrop = document.getElementById('sidebar-backdrop');
  if (!sidebar) return;

  const isOpen = typeof forceOpen === 'boolean' ? forceOpen : !sidebar.classList.contains('open');
  if (isOpen) {
    sidebar.classList.add('open');
    if (backdrop) backdrop.classList.add('active');
    document.body.classList.add('sidebar-open');
  } else {
    sidebar.classList.remove('open');
    if (backdrop) backdrop.classList.remove('active');
    document.body.classList.remove('sidebar-open');
  }
};

// Special Views: Bookmarks & Passed Modules
window.showBookmarksView = function() {
  activeSpecialView = 'bookmarks';
  document.getElementById('nav-btn-bookmarks').classList.add('active');
  document.getElementById('nav-btn-completed').classList.remove('active');
  refreshModules();
};

window.showCompletedView = function() {
  activeSpecialView = 'completed';
  document.getElementById('nav-btn-completed').classList.add('active');
  document.getElementById('nav-btn-bookmarks').classList.remove('active');
  refreshModules();
};

window.exitSpecialView = function() {
  activeSpecialView = null;
  document.getElementById('nav-btn-bookmarks').classList.remove('active');
  document.getElementById('nav-btn-completed').classList.remove('active');
  refreshModules();
};

// Range slider helper
window.updateCreditsLabel = function(val) {
  const badge = document.getElementById('credits-val-badge');
  if (!badge) return;
  if (val <= 0) {
    badge.textContent = '0 ECTS (Alle)';
  } else {
    badge.textContent = `mind. ${val} ECTS`;
  }
  localStorage.setItem(STORAGE_KEYS.MIN_CREDITS, val);
};

window.setViewMode = function(mode) {
  localStorage.setItem(STORAGE_KEYS.VIEW_MODE, mode);
  const viewInput = document.getElementById('filter-view');
  if (viewInput) {
    viewInput.value = mode;
    refreshModules();
  }
  document.querySelectorAll('.btn-view').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.view === mode);
  });
};

function updateBadges() {
  const compCount = document.getElementById('completed-count');
  if (compCount) compCount.textContent = getCompletedModules().length;

  const bkmkCount = document.getElementById('bookmarked-count');
  if (bkmkCount) bkmkCount.textContent = getBookmarkedModules().length;
}

function refreshModules() {
  const form = document.getElementById('filter-form');
  if (form) {
    // Reset offset to 0 on new filter change
    const offsetInput = document.getElementById('filter-offset');
    if (offsetInput) offsetInput.value = '0';
    htmx.trigger(form, 'submit');
  }
}

// HTMX request configuration hook: inject localStorage parameters & active special views
document.addEventListener('htmx:configRequest', function(evt) {
  const completed = getCompletedModules();
  const bookmarks = getBookmarkedModules();

  evt.detail.parameters['completed'] = completed.join(',');
  evt.detail.parameters['bookmarks'] = bookmarks.join(',');
  evt.detail.headers['X-BTU-Completed-Modules'] = completed.join(',');
  evt.detail.headers['X-BTU-Bookmarked-Modules'] = bookmarks.join(',');

  const searchInput = document.getElementById('search-input');
  if (searchInput) {
    evt.detail.parameters['q'] = searchInput.value.trim();
  }

  if (activeSpecialView === 'bookmarks') {
    evt.detail.parameters['only_bookmarked'] = 'true';
  } else if (activeSpecialView === 'completed') {
    evt.detail.parameters['only_completed'] = 'true';
  }
});

// Remove modal lock and close mobile sidebar when Escape is pressed
document.addEventListener('keydown', function(evt) {
  if (evt.key === 'Escape') {
    closeModal();
    toggleSidebar(false);
    const suggestions = document.getElementById('search-suggestions');
    if (suggestions) suggestions.style.display = 'none';
  }
});

// Reset mobile sidebar state on window resize across breakpoint
window.addEventListener('resize', function() {
  if (window.innerWidth > 960) {
    const sidebar = document.getElementById('sidebar');
    const backdrop = document.getElementById('sidebar-backdrop');
    if (sidebar) sidebar.classList.remove('open');
    if (backdrop) backdrop.classList.remove('active');
    document.body.classList.remove('sidebar-open');
  }
});

// Click outside to close combobox and search suggestions
document.addEventListener('click', function(evt) {
  const combobox = document.getElementById('combobox-dropdown');
  const comboboxWrap = document.querySelector('.combobox-container');
  if (combobox && comboboxWrap && !comboboxWrap.contains(evt.target)) {
    combobox.style.display = 'none';
    const display = document.getElementById('combobox-display');
    if (display) display.classList.remove('active');
  }

  const suggestions = document.getElementById('search-suggestions');
  const searchWrap = document.querySelector('.nav-search');
  if (suggestions && searchWrap && !searchWrap.contains(evt.target)) {
    suggestions.style.display = 'none';
  }
});

// Searchable Combobox logic & Keyboard Navigation
let highlightedComboboxIndex = -1;

function getVisibleComboboxOptions() {
  return Array.from(document.querySelectorAll('#combobox-options-list .combobox-option')).filter(opt => {
    return opt.style.display !== 'none';
  });
}

function updateHighlightedCombobox(visibleOptions) {
  document.querySelectorAll('#combobox-options-list .combobox-option').forEach(opt => {
    opt.classList.remove('highlighted');
  });

  if (highlightedComboboxIndex >= 0 && highlightedComboboxIndex < visibleOptions.length) {
    const activeOpt = visibleOptions[highlightedComboboxIndex];
    activeOpt.classList.add('highlighted');
    activeOpt.scrollIntoView({ block: 'nearest' });
  }
}

window.setHighlightedComboboxOption = function(optEl) {
  const visible = getVisibleComboboxOptions();
  highlightedComboboxIndex = visible.indexOf(optEl);
  updateHighlightedCombobox(visible);
};

window.toggleCombobox = function(event) {
  if (event) event.stopPropagation();
  const dropdown = document.getElementById('combobox-dropdown');
  const display = document.getElementById('combobox-display');
  if (!dropdown) return;

  const isOpen = dropdown.style.display === 'flex';
  dropdown.style.display = isOpen ? 'none' : 'flex';
  if (display) display.classList.toggle('active', !isOpen);

  if (!isOpen) {
    const input = document.getElementById('combobox-search');
    if (input) {
      input.value = '';
      filterComboboxOptions('');
      input.focus();
    }
    // Pre-highlight currently selected study program if visible
    const visible = getVisibleComboboxOptions();
    const currentId = document.getElementById('filter-program')?.value || '';
    const selectedIdx = visible.findIndex(opt => opt.dataset.id === currentId);
    if (selectedIdx >= 0) {
      highlightedComboboxIndex = selectedIdx;
      updateHighlightedCombobox(visible);
    } else {
      highlightedComboboxIndex = -1;
    }
  } else {
    highlightedComboboxIndex = -1;
    document.querySelectorAll('#combobox-options-list .combobox-option').forEach(opt => {
      opt.classList.remove('highlighted');
    });
  }
};

window.filterComboboxOptions = function(query) {
  const q = query.toLowerCase();
  document.querySelectorAll('#combobox-options-list .combobox-option').forEach(opt => {
    const text = (opt.dataset.title || '').toLowerCase();
    opt.style.display = text.includes(q) ? 'flex' : 'none';
    opt.classList.remove('highlighted');
  });
  highlightedComboboxIndex = -1;
};

window.selectStudyProgram = function(id, title) {
  const hiddenInput = document.getElementById('filter-program');
  const label = document.getElementById('combobox-label');
  const clearBtn = document.getElementById('btn-clear-program');

  if (hiddenInput) hiddenInput.value = id;
  if (label) label.textContent = title || 'Alle Studiengänge (Gesamtkatalog)';
  if (clearBtn) clearBtn.style.display = id ? 'inline-block' : 'none';

  localStorage.setItem(STORAGE_KEYS.PROGRAM, id);
  localStorage.setItem(STORAGE_KEYS.PROGRAM_TITLE, title || '');

  if (id) {
    trackAnonymousEvent('program_select', id, title);
  }

  // Close dropdown
  const dropdown = document.getElementById('combobox-dropdown');
  if (dropdown) dropdown.style.display = 'none';
  const display = document.getElementById('combobox-display');
  if (display) display.classList.remove('active');

  highlightedComboboxIndex = -1;
  document.querySelectorAll('#combobox-options-list .combobox-option').forEach(opt => {
    opt.classList.remove('highlighted');
  });

  // Mark selected option
  document.querySelectorAll('.combobox-option').forEach(opt => {
    opt.classList.toggle('selected', opt.dataset.id === id);
  });

  // On mobile screens, auto-close sidebar drawer on selection
  if (window.innerWidth <= 960) {
    toggleSidebar(false);
  }

  refreshModules();
};

window.clearStudyProgram = function(event) {
  if (event) event.stopPropagation();
  selectStudyProgram('', '');
};

// Initialize on page load
document.addEventListener('DOMContentLoaded', function() {
  const form = document.getElementById('filter-form');
  if (!form) return;

  // Restore Program
  const savedProg = localStorage.getItem(STORAGE_KEYS.PROGRAM) || '';
  const savedTitle = localStorage.getItem(STORAGE_KEYS.PROGRAM_TITLE) || '';
  selectStudyProgram(savedProg, savedTitle);

  // Keyboard navigation on combobox search input
  const comboboxSearch = document.getElementById('combobox-search');
  if (comboboxSearch) {
    comboboxSearch.addEventListener('keydown', function(evt) {
      const visible = getVisibleComboboxOptions();
      if (visible.length === 0) {
        if (evt.key === 'Escape') {
          const dropdown = document.getElementById('combobox-dropdown');
          if (dropdown) dropdown.style.display = 'none';
          const display = document.getElementById('combobox-display');
          if (display) display.classList.remove('active');
        }
        return;
      }

      if (evt.key === 'ArrowDown') {
        evt.preventDefault();
        if (highlightedComboboxIndex < visible.length - 1) {
          highlightedComboboxIndex++;
        } else {
          highlightedComboboxIndex = 0; // Wrap around to top
        }
        updateHighlightedCombobox(visible);
      } else if (evt.key === 'ArrowUp') {
        evt.preventDefault();
        if (highlightedComboboxIndex > 0) {
          highlightedComboboxIndex--;
        } else if (highlightedComboboxIndex === 0) {
          highlightedComboboxIndex = -1; // Unselect back to input
        } else {
          highlightedComboboxIndex = visible.length - 1; // Wrap around to bottom
        }
        updateHighlightedCombobox(visible);
      } else if (evt.key === 'Enter') {
        evt.preventDefault();
        let targetOption = null;
        if (highlightedComboboxIndex >= 0 && highlightedComboboxIndex < visible.length) {
          targetOption = visible[highlightedComboboxIndex];
        } else if (visible.length === 1) {
          targetOption = visible[0];
        } else if (visible.length > 0) {
          targetOption = visible[0];
        }

        if (targetOption) {
          selectStudyProgram(targetOption.dataset.id, targetOption.dataset.title);
        }
      } else if (evt.key === 'Escape') {
        const dropdown = document.getElementById('combobox-dropdown');
        if (dropdown) dropdown.style.display = 'none';
        const display = document.getElementById('combobox-display');
        if (display) display.classList.remove('active');
        highlightedComboboxIndex = -1;
        updateHighlightedCombobox(visible);
      }
    });
  }

  // Restore Turnus
  const savedTurnus = localStorage.getItem(STORAGE_KEYS.TURNUS);
  if (savedTurnus) {
    const radio = document.querySelector(`input[name="turnus"][value="${savedTurnus}"]`);
    if (radio) radio.checked = true;
  }
  document.querySelectorAll('input[name="turnus"]').forEach(r => {
    r.addEventListener('change', () => {
      if (r.checked) localStorage.setItem(STORAGE_KEYS.TURNUS, r.value);
    });
  });

  // Restore Only FÜS
  const savedFUES = localStorage.getItem(STORAGE_KEYS.ONLY_FUES);
  const fuesCheck = document.getElementById('filter-fues');
  if (savedFUES !== null && fuesCheck) {
    fuesCheck.checked = savedFUES === 'true';
  }
  if (fuesCheck) {
    fuesCheck.addEventListener('change', () => {
      localStorage.setItem(STORAGE_KEYS.ONLY_FUES, fuesCheck.checked);
    });
  }

  // Restore Hide Phase Out
  const savedHidePhaseOut = localStorage.getItem(STORAGE_KEYS.HIDE_PHASE_OUT);
  const phaseOutCheck = document.getElementById('filter-hide-phaseout');
  if (savedHidePhaseOut !== null && phaseOutCheck) {
    phaseOutCheck.checked = savedHidePhaseOut === 'true';
  }
  if (phaseOutCheck) {
    phaseOutCheck.addEventListener('change', () => {
      localStorage.setItem(STORAGE_KEYS.HIDE_PHASE_OUT, phaseOutCheck.checked);
    });
  }

  // Restore Prerequisites Met
  const savedPrereqs = localStorage.getItem(STORAGE_KEYS.PREREQS_MET);
  const prereqCheck = document.getElementById('filter-prereqs');
  if (savedPrereqs !== null && prereqCheck) {
    prereqCheck.checked = savedPrereqs === 'true';
  }
  if (prereqCheck) {
    prereqCheck.addEventListener('change', () => {
      localStorage.setItem(STORAGE_KEYS.PREREQS_MET, prereqCheck.checked);
    });
  }

  // Restore Min Credits Slider
  const savedCredits = localStorage.getItem(STORAGE_KEYS.MIN_CREDITS);
  const creditsSlider = document.getElementById('filter-min-credits');
  if (savedCredits !== null && creditsSlider) {
    creditsSlider.value = savedCredits;
    updateCreditsLabel(savedCredits);
  }

  // Restore View Mode
  const savedView = localStorage.getItem(STORAGE_KEYS.VIEW_MODE) || 'grid';
  const viewInput = document.getElementById('filter-view');
  if (viewInput) viewInput.value = savedView;
  document.querySelectorAll('.btn-view').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.view === savedView);
  });

  // Synchronize active filtered modules from the server response
  function syncFilterModules() {
    const dataEl = document.getElementById('current-filter-modules-data');
    if (dataEl) {
      try {
        window.activeFilterModules = JSON.parse(dataEl.textContent);
      } catch(e) {
        window.activeFilterModules = [];
      }
    }
  }

  document.body.addEventListener('htmx:afterSwap', function(evt) {
    if (evt.target && evt.target.id === 'modules-view') {
      syncFilterModules();
    }
  });

  // Relevance & Fuzzy scoring algorithm
  function scoreModule(item, queryNorm) {
    const idNorm = (item.id || '').toLowerCase();
    const titleDENorm = (item.title_de || '').toLowerCase();
    const titleENNorm = (item.title_en || '').toLowerCase();

    if (idNorm === queryNorm) return 10000;
    if (idNorm.startsWith(queryNorm)) return 6000 + (queryNorm.length / idNorm.length) * 1000;
    if (idNorm.includes(queryNorm)) return 4500;
    if (titleDENorm === queryNorm) return 4000;
    if (titleDENorm.startsWith(queryNorm)) return 3000 + (queryNorm.length / titleDENorm.length) * 500;

    let score = 0;
    const words = titleDENorm.split(/[\s+\-/.,()]+/);
    for (const w of words) {
      if (w === queryNorm) {
        score = Math.max(score, 2500);
      } else if (w.startsWith(queryNorm)) {
        score = Math.max(score, 2000 + (queryNorm.length / w.length) * 400);
      }
    }

    const subIdx = titleDENorm.indexOf(queryNorm);
    if (subIdx > -1) {
      score = Math.max(score, 1200 - Math.min(subIdx, 40) * 10);
    }

    if (titleENNorm.startsWith(queryNorm)) {
      score = Math.max(score, 1500);
    } else if (titleENNorm.includes(queryNorm)) {
      score = Math.max(score, 800);
    }

    const tokens = queryNorm.split(/\s+/).filter(Boolean);
    if (tokens.length > 1) {
      let allMatched = true;
      let tokenScore = 1500;
      for (const t of tokens) {
        if (titleDENorm.includes(t) || idNorm.includes(t) || titleENNorm.includes(t)) {
          tokenScore += 300;
        } else {
          allMatched = false;
        }
      }
      if (allMatched) score = Math.max(score, tokenScore);
    } else if (score === 0 && queryNorm.length >= 3) {
      let tIdx = 0;
      let matchCount = 0;
      for (let i = 0; i < titleDENorm.length && tIdx < queryNorm.length; i++) {
        if (titleDENorm[i] === queryNorm[tIdx]) {
          matchCount++;
          tIdx++;
        }
      }
      if (tIdx === queryNorm.length) {
        score = Math.max(score, 600 + (matchCount / titleDENorm.length) * 200);
      }
    }

    return score;
  }

  // Instant Client-Side Fuzzy Autocomplete Suggestions & Keyboard Navigation
  const searchInput = document.getElementById('search-input');
  const suggestionsBox = document.getElementById('search-suggestions');
  let highlightedSuggestionIndex = -1;

  function updateHighlightedSuggestion(items) {
    items.forEach((item, idx) => {
      if (idx === highlightedSuggestionIndex) {
        item.classList.add('selected');
        item.scrollIntoView({ block: 'nearest' });
      } else {
        item.classList.remove('selected');
      }
    });
  }

  function hideSuggestions() {
    if (suggestionsBox) suggestionsBox.style.display = 'none';
    highlightedSuggestionIndex = -1;
  }

  window.selectSuggestionItem = function(moduleId) {
    hideSuggestions();
    openModal(moduleId);
  };

  window.setHighlightedSuggestion = function(idx) {
    highlightedSuggestionIndex = idx;
    if (suggestionsBox) {
      const items = suggestionsBox.querySelectorAll('.suggestion-item');
      updateHighlightedSuggestion(items);
    }
  };

  if (searchInput && suggestionsBox) {
    // Typing only updates the live autocomplete preview without modifying the main module list
    searchInput.addEventListener('input', function() {
      const rawVal = searchInput.value.trim();
      highlightedSuggestionIndex = -1;

      if (rawVal.length < 1) {
        hideSuggestions();
        return;
      }

      const qNorm = rawVal.toLowerCase();
      const modules = window.activeFilterModules || [];
      if (modules.length === 0) {
        suggestionsBox.innerHTML = '<div class="suggestion-empty">Keine Module in den aktiven Filtern vorhanden</div>';
        suggestionsBox.style.display = 'block';
        return;
      }

      const scored = [];
      for (const m of modules) {
        const sc = scoreModule(m, qNorm);
        if (sc > 0) {
          scored.push({ item: m, score: sc });
        }
      }

      scored.sort((a, b) => b.score - a.score);
      const topResults = scored.slice(0, 8);

      if (topResults.length === 0) {
        suggestionsBox.innerHTML = '<div class="suggestion-empty">Keine passenden Module in deinen Filtern</div>';
        suggestionsBox.style.display = 'block';
        return;
      }

      let html = '';
      topResults.forEach((res, idx) => {
        const it = res.item;
        const cred = it.credits ? it.credits.toFixed(1) : '0';
        const turnus = it.turnus || 'Kein Turnus';
        const fuesBadge = it.is_fues ? '<span class="badge badge-fues" style="font-size:0.65rem; padding:0.1rem 0.3rem;">FÜS</span>' : '';
        const phaseBadge = it.is_phase ? '<span class="badge badge-phaseout" style="font-size:0.65rem; padding:0.1rem 0.3rem;">Auslaufend</span>' : '';

        html += `
          <div class="suggestion-item" data-id="${it.id}" data-index="${idx}" onmouseenter="setHighlightedSuggestion(${idx});" onclick="selectSuggestionItem('${it.id}');">
            <div class="suggestion-main">
              <span class="badge badge-id">${it.id}</span>
              <strong class="suggestion-title">${escapeHTML(it.title_de || '')}</strong>
              ${fuesBadge}
              ${phaseBadge}
            </div>
            <div class="suggestion-meta">${cred} ECTS • ${escapeHTML(turnus)}</div>
          </div>
        `;
      });

      suggestionsBox.innerHTML = html;
      suggestionsBox.style.display = 'block';
    });

    // Arrow keys & Enter key navigation
    searchInput.addEventListener('keydown', function(evt) {
      const items = suggestionsBox.querySelectorAll('.suggestion-item');
      const isBoxVisible = suggestionsBox.style.display !== 'none' && items.length > 0;

      if (evt.key === 'ArrowDown') {
        if (!isBoxVisible) return;
        evt.preventDefault();
        if (highlightedSuggestionIndex < items.length - 1) {
          highlightedSuggestionIndex++;
        } else {
          highlightedSuggestionIndex = 0; // Wrap around to top
        }
        updateHighlightedSuggestion(items);
      } else if (evt.key === 'ArrowUp') {
        if (!isBoxVisible) return;
        evt.preventDefault();
        if (highlightedSuggestionIndex > 0) {
          highlightedSuggestionIndex--;
        } else if (highlightedSuggestionIndex === 0) {
          highlightedSuggestionIndex = -1; // Unselect back to typing in input
        } else {
          highlightedSuggestionIndex = items.length - 1; // Wrap around to bottom
        }
        updateHighlightedSuggestion(items);
      } else if (evt.key === 'Enter') {
        evt.preventDefault();
        if (isBoxVisible && highlightedSuggestionIndex >= 0 && items[highlightedSuggestionIndex]) {
          // If a suggestion is highlighted, open module modal dialog directly!
          const modId = items[highlightedSuggestionIndex].dataset.id;
          hideSuggestions();
          openModal(modId);
        } else {
          // If no suggestion is highlighted, Enter updates the main module list!
          hideSuggestions();
          refreshModules();
        }
      } else if (evt.key === 'Escape') {
        hideSuggestions();
      }
    });

    // Native clear button 'search' event on <input type="search">
    searchInput.addEventListener('search', function() {
      hideSuggestions();
      refreshModules();
    });

    // Re-show suggestions if input is focused and contains query
    searchInput.addEventListener('focus', function() {
      if (searchInput.value.trim().length > 0 && suggestionsBox.children.length > 0) {
        suggestionsBox.style.display = 'block';
      }
    });
  }

  function escapeHTML(str) {
    return str.replace(/[&<>'"]/g, 
      tag => ({
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        "'": '&#39;',
        '"': '&quot;'
      }[tag] || tag)
    );
  }

  // Update initial badges
  updateBadges();

  // Track initial anonymous page view
  trackAnonymousEvent('view');

  // Trigger initial fetch
  refreshModules();
});
