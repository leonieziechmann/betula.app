// BTU Smart Modulkatalog - LocalStorage Synchronizer & Interactive UI Logic

const STORAGE_KEYS = {
  PROGRAM: 'btu_selected_program',
  PROGRAM_GROUP: 'btu_selected_program_group',
  PROGRAM_TITLE: 'btu_selected_program_title',
  PO_ID: 'btu_selected_po_id',
  TURNUS: 'btu_target_semester',
  CAMPUSES: 'btu_campuses',
  CAMPUS_STRICT: 'btu_campus_strict',
  LIMITATION: 'btu_limitation',
  FUES: 'btu_fues',
  INSTRUCTORS: 'btu_whitelisted_instructors',
  LANG_DE: 'btu_lang_de',
  LANG_EN: 'btu_lang_en',
  COMPLETED: 'btu_completed_modules',
  BOOKMARKS: 'btu_bookmarked_modules',
  ONLY_FUES: 'btu_only_fues',
  HIDE_PHASE_OUT: 'btu_hide_phase_out',
  VIEW_MODE: 'btu_view_mode',
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

window.toggleAccordion = function(id) {
  const section = document.getElementById(id);
  if (!section) return;
  const isCollapsed = section.classList.toggle('collapsed');
  const btn = section.querySelector('.accordion-header');
  if (btn) {
    btn.setAttribute('aria-expanded', String(!isCollapsed));
  }
};

window.setViewMode = function(mode) {
  // Legacy stub - table view is now the permanent view mode
};

function updateBadges() {
  const compCount = document.getElementById('completed-count');
  if (compCount) compCount.textContent = getCompletedModules().length;

  const bkmkCount = document.getElementById('bookmarked-count');
  if (bkmkCount) bkmkCount.textContent = getBookmarkedModules().length;
}

let refreshDebounceTimer = null;
function refreshModules() {
  clearTimeout(refreshDebounceTimer);
  refreshDebounceTimer = setTimeout(() => {
    const form = document.getElementById('filter-form');
    if (form) {
      const offsetInput = document.getElementById('filter-offset');
      if (offsetInput) offsetInput.value = '0';
      htmx.trigger(form, 'submit');
    }
    updateActiveFilterCount();
  }, 25);
}

function updateActiveFilterCount() {
  let count = 0;
  const prog = document.getElementById('filter-program');
  if (prog && prog.value) count++;

  if (typeof selectedTurnuses !== 'undefined' && selectedTurnuses.length > 0) count++;

  const credits = document.getElementById('filter-min-credits');
  if (credits && parseFloat(credits.value) > 0) count++;

  const checkedCampuses = document.querySelectorAll('input[name="campus"]:checked');
  if (checkedCampuses.length > 0) count += checkedCampuses.length;

  const strict = document.getElementById('filter-campus-strict');
  if (strict && strict.checked) count++;

  const limit = document.getElementById('filter-limitation');
  if (limit && limit.value && limit.value !== 'ja') count++;

  const fues = document.getElementById('filter-fues');
  if (fues && fues.value && fues.value !== 'inkl') count++;

  if (typeof selectedInstructors !== 'undefined' && selectedInstructors.length > 0) {
    count += selectedInstructors.length;
  }

  const phaseOut = document.getElementById('filter-hide-phaseout');
  if (phaseOut && !phaseOut.checked) count++;

  const de = document.getElementById('filter-lang-de');
  const en = document.getElementById('filter-lang-en');
  if (de && !de.checked) count++;
  if (en && en.checked) count++;

  const badge = document.getElementById('active-filters-count');
  if (badge) {
    if (count > 0) {
      badge.textContent = `${count} aktiv`;
      badge.style.display = 'inline-block';
    } else {
      badge.style.display = 'none';
    }
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
    const profMenu = document.getElementById('prof-dropdown-menu');
    if (profMenu) profMenu.style.display = 'none';
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

// Click outside to close combobox, search suggestions, and prof dropdown
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

  const profMenu = document.getElementById('prof-dropdown-menu');
  const profWrap = document.querySelector('.prof-search-wrapper');
  if (profMenu && profWrap && !profWrap.contains(evt.target)) {
    profMenu.style.display = 'none';
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

let programGroups = [];
function initProgramGroups() {
  try {
    const el = document.getElementById('program-groups-data');
    if (!el) return;
    let raw = el.textContent ? el.textContent.trim() : '';
    let parsed = JSON.parse(raw || '[]');
    if (typeof parsed === 'string') {
      parsed = JSON.parse(parsed);
    }
    programGroups = Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    console.error("Failed to parse program groups:", e);
    programGroups = [];
  }
}

window.onProgramOptionClick = function(el) {
  const key = el.getAttribute('data-key') || '';
  const title = el.getAttribute('data-title') || '';
  selectStudyProgramGroup(key, title);
};

window.selectStudyProgramGroup = function(groupKey, title, preferredPoId, shouldRefresh = true) {
  if (programGroups.length === 0) initProgramGroups();

  const hiddenInput = document.getElementById('filter-program');
  const label = document.getElementById('combobox-label');
  const clearBtn = document.getElementById('btn-clear-program');
  const poDisplay = document.getElementById('po-display');
  const poDisplayText = document.getElementById('po-display-text');
  const poSelect = document.getElementById('po-select');

  localStorage.setItem(STORAGE_KEYS.PROGRAM_GROUP, groupKey || '');
  localStorage.setItem(STORAGE_KEYS.PROGRAM_TITLE, title || '');

  if (!groupKey) {
    if (hiddenInput) hiddenInput.value = '';
    if (label) label.textContent = 'Alle Studiengänge';
    if (clearBtn) clearBtn.style.display = 'none';
    if (poDisplayText) poDisplayText.textContent = '-';
    if (poDisplay) poDisplay.style.display = 'flex';
    if (poSelect) poSelect.style.display = 'none';
    localStorage.removeItem(STORAGE_KEYS.PROGRAM);
    localStorage.removeItem(STORAGE_KEYS.PO_ID);
  } else {
    const grp = programGroups.find(g => g.key === groupKey);
    if (label) label.textContent = title || groupKey;
    if (clearBtn) clearBtn.style.display = 'inline-block';

    if (grp && grp.pos && grp.pos.length > 1) {
      // Multiple POs: show interactive dropdown
      if (poDisplay) poDisplay.style.display = 'none';
      if (poSelect) {
        poSelect.innerHTML = '';
        let targetPoId = preferredPoId || '';
        grp.pos.forEach(po => {
          const opt = document.createElement('option');
          opt.value = po.id;
          const poLabel = po.po_version ? `PO ${po.po_version}` : 'PO Standard';
          opt.textContent = `${poLabel} (${po.count})`;
          poSelect.appendChild(opt);
          if (!targetPoId) targetPoId = po.id;
        });

        const preferredMatch = preferredPoId ? grp.pos.find(p => p.id === preferredPoId || (p.related_ids && p.related_ids.includes(preferredPoId))) : null;
        if (preferredMatch) {
          poSelect.value = preferredMatch.id;
        } else if (targetPoId) {
          poSelect.value = targetPoId;
        }

        poSelect.style.display = 'block';
        if (hiddenInput) hiddenInput.value = poSelect.value;
        localStorage.setItem(STORAGE_KEYS.PROGRAM, poSelect.value);
        localStorage.setItem(STORAGE_KEYS.PO_ID, poSelect.value);
      }
    } else if (grp && grp.pos && grp.pos.length === 1) {
      // Exactly 1 PO: show read-only indicator badge
      const singlePO = grp.pos[0];
      if (poSelect) poSelect.style.display = 'none';
      if (poDisplay) poDisplay.style.display = 'flex';
      const shortPo = singlePO.po_version ? `PO ${singlePO.po_version}` : 'PO Standard';
      if (poDisplayText) poDisplayText.textContent = shortPo;
      if (hiddenInput) hiddenInput.value = singlePO.id;
      localStorage.setItem(STORAGE_KEYS.PROGRAM, singlePO.id);
      localStorage.setItem(STORAGE_KEYS.PO_ID, singlePO.id);
    } else {
      if (poDisplayText) poDisplayText.textContent = '-';
      if (poDisplay) poDisplay.style.display = 'flex';
      if (poSelect) poSelect.style.display = 'none';
      if (hiddenInput) hiddenInput.value = '';
    }
  }

  // Close dropdown
  const dropdown = document.getElementById('combobox-dropdown');
  if (dropdown) dropdown.style.display = 'none';
  const display = document.getElementById('combobox-display');
  if (display) display.classList.remove('active');

  highlightedComboboxIndex = -1;
  document.querySelectorAll('#combobox-options-list .combobox-option').forEach(opt => {
    opt.classList.remove('highlighted');
    opt.classList.toggle('selected', opt.dataset.key === groupKey);
  });

  if (window.innerWidth <= 960) {
    toggleSidebar(false);
  }

  if (shouldRefresh) {
    refreshModules();
  }
};

window.onPOSelectChanged = function(poId) {
  const hiddenInput = document.getElementById('filter-program');
  if (hiddenInput) hiddenInput.value = poId;
  localStorage.setItem(STORAGE_KEYS.PROGRAM, poId);
  localStorage.setItem(STORAGE_KEYS.PO_ID, poId);
  refreshModules();
};

window.clearStudyProgram = function(event) {
  if (event) event.stopPropagation();
  selectStudyProgramGroup('', '');
};

// Multi-select Semester Turnus logic
let selectedTurnuses = []; // e.g. ['sose_even', 'wise_odd', 'sporadic']

function renderTurnusButtons() {
  const hiddenContainer = document.getElementById('turnus-hidden-inputs');
  const clearBtn = document.getElementById('btn-clear-turnus');

  // Update hidden inputs for form serialization
  if (hiddenContainer) {
    hiddenContainer.innerHTML = '';
    selectedTurnuses.forEach(t => {
      const input = document.createElement('input');
      input.type = 'hidden';
      input.name = 'turnus';
      input.value = t;
      hiddenContainer.appendChild(input);
    });
  }

  // Update button active states
  const hasSoseEven = selectedTurnuses.includes('sose_even');
  const hasSoseOdd = selectedTurnuses.includes('sose_odd');
  const hasWiseEven = selectedTurnuses.includes('wise_even');
  const hasWiseOdd = selectedTurnuses.includes('wise_odd');
  const hasSporadic = selectedTurnuses.includes('sporadic');

  const btnSose = document.getElementById('btn-turnus-sose');
  const btnSoseEven = document.getElementById('btn-turnus-sose_even');
  const btnSoseOdd = document.getElementById('btn-turnus-sose_odd');

  const btnWise = document.getElementById('btn-turnus-wise');
  const btnWiseEven = document.getElementById('btn-turnus-wise_even');
  const btnWiseOdd = document.getElementById('btn-turnus-wise_odd');

  const btnSporadic = document.getElementById('btn-turnus-sporadic');

  if (btnSoseEven) btnSoseEven.classList.toggle('active', hasSoseEven);
  if (btnSoseOdd) btnSoseOdd.classList.toggle('active', hasSoseOdd);
  if (btnSose) {
    btnSose.classList.toggle('active', hasSoseEven && hasSoseOdd);
  }

  if (btnWiseEven) btnWiseEven.classList.toggle('active', hasWiseEven);
  if (btnWiseOdd) btnWiseOdd.classList.toggle('active', hasWiseOdd);
  if (btnWise) {
    btnWise.classList.toggle('active', hasWiseEven && hasWiseOdd);
  }

  if (btnSporadic) btnSporadic.classList.toggle('active', hasSporadic);

  if (clearBtn) {
    clearBtn.style.display = selectedTurnuses.length > 0 ? 'inline-block' : 'none';
  }

  updateActiveFilterCount();
}

window.toggleTurnusToken = function(token, shouldRefresh = true) {
  const idx = selectedTurnuses.indexOf(token);
  if (idx > -1) {
    selectedTurnuses.splice(idx, 1);
  } else {
    selectedTurnuses.push(token);
  }

  try {
    localStorage.setItem(STORAGE_KEYS.TURNUS, JSON.stringify(selectedTurnuses));
  } catch(e) {}

  renderTurnusButtons();

  if (shouldRefresh) {
    refreshModules();
  }
};

window.toggleTurnusMain = function(category, shouldRefresh = true) {
  const tokens = category === 'sose' ? ['sose_even', 'sose_odd'] : ['wise_even', 'wise_odd'];
  const allActive = tokens.every(t => selectedTurnuses.includes(t));

  if (allActive) {
    selectedTurnuses = selectedTurnuses.filter(t => !tokens.includes(t));
  } else {
    tokens.forEach(t => {
      if (!selectedTurnuses.includes(t)) {
        selectedTurnuses.push(t);
      }
    });
  }

  try {
    localStorage.setItem(STORAGE_KEYS.TURNUS, JSON.stringify(selectedTurnuses));
  } catch(e) {}

  renderTurnusButtons();

  if (shouldRefresh) {
    refreshModules();
  }
};

window.clearTurnusFilter = function(shouldRefresh = true) {
  selectedTurnuses = [];
  try {
    localStorage.removeItem(STORAGE_KEYS.TURNUS);
  } catch(e) {}

  renderTurnusButtons();

  if (shouldRefresh) {
    refreshModules();
  }
};

// Limitation segmented control helper
window.setLimitationFilter = function(val, shouldRefresh = true) {
  const hidden = document.getElementById('filter-limitation');
  if (hidden) hidden.value = val;
  localStorage.setItem(STORAGE_KEYS.LIMITATION, val);

  document.querySelectorAll('#limitation-control .segmented-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.val === val);
  });

  if (shouldRefresh) {
    refreshModules();
  }
};

// FÜS 3-way segmented control helper (exkl, inkl, nur)
window.setFUESFilter = function(val, shouldRefresh = true) {
  const hidden = document.getElementById('filter-fues');
  if (hidden) hidden.value = val;
  localStorage.setItem(STORAGE_KEYS.FUES, val);

  document.querySelectorAll('#fues-control .segmented-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.val === val);
  });

  if (shouldRefresh) {
    refreshModules();
  }
};

// Dozierende (Professoren) Whitelist Filter
let allInstructors = [];
let selectedInstructors = [];
let highlightedProfIndex = -1;

function initInstructors() {
  try {
    const el = document.getElementById('instructors-data');
    if (!el) return;
    let raw = el.textContent ? el.textContent.trim() : '';
    let parsed = JSON.parse(raw || '[]');
    if (typeof parsed === 'string') {
      parsed = JSON.parse(parsed);
    }
    allInstructors = Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    console.error("Failed to parse instructors:", e);
    allInstructors = [];
  }
}

function renderSelectedProfChips() {
  const chipsContainer = document.getElementById('prof-selected-chips');
  const hiddenContainer = document.getElementById('prof-hidden-inputs');
  const clearBtn = document.getElementById('btn-clear-profs');

  if (hiddenContainer) {
    hiddenContainer.innerHTML = '';
    selectedInstructors.forEach(name => {
      const input = document.createElement('input');
      input.type = 'hidden';
      input.name = 'instructor';
      input.value = name;
      hiddenContainer.appendChild(input);
    });
  }

  if (chipsContainer) {
    chipsContainer.innerHTML = '';
    selectedInstructors.forEach(name => {
      const inst = allInstructors.find(i => i.name === name);
      const title = inst ? inst.title : '';

      const chip = document.createElement('div');
      chip.className = 'prof-chip';
      chip.title = inst ? inst.fullname : name;

      const nameSpan = document.createElement('div');
      nameSpan.className = 'prof-chip-name';
      if (title) {
        const titleSpan = document.createElement('span');
        titleSpan.className = 'prof-chip-title';
        titleSpan.textContent = title;
        nameSpan.appendChild(titleSpan);
      }
      const textSpan = document.createElement('span');
      textSpan.className = 'prof-chip-text';
      textSpan.textContent = name;
      nameSpan.appendChild(textSpan);
      chip.appendChild(nameSpan);

      const removeBtn = document.createElement('button');
      removeBtn.type = 'button';
      removeBtn.className = 'prof-chip-remove';
      removeBtn.setAttribute('aria-label', `${name} entfernen`);
      removeBtn.title = `${name} entfernen`;
      removeBtn.innerHTML = '&times;';
      removeBtn.onclick = function(e) {
        e.stopPropagation();
        window.removeInstructor(name);
      };
      chip.appendChild(removeBtn);

      chipsContainer.appendChild(chip);
    });
  }

  if (clearBtn) {
    clearBtn.style.display = selectedInstructors.length > 0 ? 'inline-block' : 'none';
  }

  updateActiveFilterCount();
}

window.addInstructor = function(name, shouldRefresh = true) {
  if (!name) return;
  if (!selectedInstructors.includes(name)) {
    selectedInstructors.push(name);
    try {
      localStorage.setItem(STORAGE_KEYS.INSTRUCTORS, JSON.stringify(selectedInstructors));
    } catch(e) {}
    renderSelectedProfChips();
  }

  const input = document.getElementById('prof-search-input');
  if (input) input.value = '';
  hideProfDropdown();

  if (shouldRefresh) {
    refreshModules();
  }
};

window.removeInstructor = function(name) {
  selectedInstructors = selectedInstructors.filter(n => n !== name);
  try {
    localStorage.setItem(STORAGE_KEYS.INSTRUCTORS, JSON.stringify(selectedInstructors));
  } catch(e) {}
  renderSelectedProfChips();
  refreshModules();
};

window.clearProfWhitelist = function() {
  selectedInstructors = [];
  try {
    localStorage.removeItem(STORAGE_KEYS.INSTRUCTORS);
  } catch(e) {}
  renderSelectedProfChips();
  refreshModules();
};

function getVisibleProfItems() {
  return Array.from(document.querySelectorAll('#prof-dropdown-menu .prof-dropdown-item'));
}

function updateHighlightedProf(visibleItems) {
  visibleItems.forEach((item, idx) => {
    if (idx === highlightedProfIndex) {
      item.classList.add('highlighted');
      item.scrollIntoView({ block: 'nearest' });
    } else {
      item.classList.remove('highlighted');
    }
  });
}

function hideProfDropdown() {
  const menu = document.getElementById('prof-dropdown-menu');
  if (menu) menu.style.display = 'none';
  highlightedProfIndex = -1;
}

window.onProfSearchInput = function(query) {
  const menu = document.getElementById('prof-dropdown-menu');
  if (!menu) return;
  const q = (query || '').trim().toLowerCase();
  if (q.length < 1) {
    hideProfDropdown();
    return;
  }

  if (allInstructors.length === 0) initInstructors();

  const matches = allInstructors.filter(inst => {
    if (selectedInstructors.includes(inst.name)) return false;
    const nameLow = (inst.name || '').toLowerCase();
    const fullLow = (inst.fullname || '').toLowerCase();
    const titleLow = (inst.title || '').toLowerCase();
    return nameLow.includes(q) || fullLow.includes(q) || titleLow.includes(q);
  }).slice(0, 15);

  if (matches.length === 0) {
    menu.innerHTML = '<div class="suggestion-empty" style="padding:0.6rem;">Keine Dozierenden gefunden</div>';
    menu.style.display = 'flex';
    highlightedProfIndex = -1;
    return;
  }

  menu.innerHTML = '';
  matches.forEach((inst, idx) => {
    const item = document.createElement('div');
    item.className = 'prof-dropdown-item';
    item.dataset.name = inst.name;
    item.dataset.index = idx;

    const nameEl = document.createElement('div');
    nameEl.className = 'prof-dropdown-name';
    nameEl.textContent = inst.name;
    item.appendChild(nameEl);

    if (inst.title) {
      const titleEl = document.createElement('div');
      titleEl.className = 'prof-dropdown-title';
      titleEl.textContent = inst.title;
      item.appendChild(titleEl);
    }

    item.onmouseenter = function() {
      highlightedProfIndex = idx;
      updateHighlightedProf(getVisibleProfItems());
    };
    item.onclick = function() {
      window.addInstructor(inst.name);
    };

    menu.appendChild(item);
  });

  menu.style.display = 'flex';
  highlightedProfIndex = -1;
};

window.onProfSearchFocus = function() {
  const input = document.getElementById('prof-search-input');
  if (input && input.value.trim().length > 0) {
    window.onProfSearchInput(input.value);
  }
};

function updateCampusChips() {
  document.querySelectorAll('.campus-chip').forEach(chip => {
    const input = chip.querySelector('input[type="checkbox"]');
    if (input) {
      chip.classList.toggle('active', input.checked);
    }
  });
}

function updateLanguageChips() {
  document.querySelectorAll('.lang-chip').forEach(chip => {
    const input = chip.querySelector('input[type="checkbox"]');
    if (input) {
      chip.classList.toggle('active', input.checked);
    }
  });
}

window.resetAllFilters = function() {
  localStorage.removeItem(STORAGE_KEYS.PROGRAM);
  localStorage.removeItem(STORAGE_KEYS.PROGRAM_GROUP);
  localStorage.removeItem(STORAGE_KEYS.PROGRAM_TITLE);
  localStorage.removeItem(STORAGE_KEYS.PO_ID);
  localStorage.removeItem(STORAGE_KEYS.TURNUS);
  localStorage.removeItem(STORAGE_KEYS.CAMPUSES);
  localStorage.removeItem(STORAGE_KEYS.CAMPUS_STRICT);
  localStorage.removeItem(STORAGE_KEYS.LIMITATION);
  localStorage.removeItem(STORAGE_KEYS.FUES);
  localStorage.removeItem(STORAGE_KEYS.INSTRUCTORS);
  localStorage.removeItem(STORAGE_KEYS.LANG_DE);
  localStorage.removeItem(STORAGE_KEYS.LANG_EN);
  localStorage.removeItem(STORAGE_KEYS.ONLY_FUES);
  localStorage.removeItem(STORAGE_KEYS.HIDE_PHASE_OUT);
  localStorage.removeItem(STORAGE_KEYS.MIN_CREDITS);

  // Reset Program & PO
  selectStudyProgramGroup('', '', '', false);

  // Reset Turnus
  selectedTurnuses = [];
  renderTurnusButtons();

  // Reset Credits
  const creditsSlider = document.getElementById('filter-min-credits');
  if (creditsSlider) {
    creditsSlider.value = '0';
    updateCreditsLabel('0');
  }

  // Reset Campuses
  document.querySelectorAll('input[name="campus"]').forEach(chk => {
    chk.checked = false;
  });
  updateCampusChips();

  // Reset Campus Strict
  const strictCheck = document.getElementById('filter-campus-strict');
  if (strictCheck) strictCheck.checked = false;

  // Reset Limitation
  setLimitationFilter('ja', false);

  // Reset FÜS
  setFUESFilter('inkl', false);

  // Reset Professoren Whitelist
  selectedInstructors = [];
  renderSelectedProfChips();

  // Reset Toggles
  const phaseOutCheck = document.getElementById('filter-hide-phaseout');
  if (phaseOutCheck) phaseOutCheck.checked = true;

  // Reset Languages
  const deCheck = document.getElementById('filter-lang-de');
  if (deCheck) deCheck.checked = true;
  const enCheck = document.getElementById('filter-lang-en');
  if (enCheck) enCheck.checked = false;
  updateLanguageChips();

  // Reset search
  const searchInput = document.getElementById('search-input');
  if (searchInput) searchInput.value = '';

  refreshModules();
};

// Initialize on page load
document.addEventListener('DOMContentLoaded', function() {
  const form = document.getElementById('filter-form');
  if (!form) return;

  initProgramGroups();
  initInstructors();

  // Restore Program & PO (without refresh during init)
  const savedGroup = localStorage.getItem(STORAGE_KEYS.PROGRAM_GROUP) || '';
  const savedTitle = localStorage.getItem(STORAGE_KEYS.PROGRAM_TITLE) || '';
  const savedPoId = localStorage.getItem(STORAGE_KEYS.PO_ID) || '';
  selectStudyProgramGroup(savedGroup, savedTitle, savedPoId, false);

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
          selectStudyProgramGroup(targetOption.dataset.key, targetOption.dataset.title);
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

  // Restore Turnus (without refresh during init)
  try {
    const savedTurnusRaw = localStorage.getItem(STORAGE_KEYS.TURNUS);
    if (savedTurnusRaw) {
      let parsed = JSON.parse(savedTurnusRaw);
      if (Array.isArray(parsed)) {
        selectedTurnuses = parsed;
      } else if (typeof parsed === 'string' && parsed !== 'all') {
        selectedTurnuses = [parsed];
      }
    } else {
      selectedTurnuses = [];
    }
  } catch(e) {
    const savedTurnusRaw = localStorage.getItem(STORAGE_KEYS.TURNUS);
    if (savedTurnusRaw && savedTurnusRaw !== 'all') {
      selectedTurnuses = [savedTurnusRaw];
    } else {
      selectedTurnuses = [];
    }
  }
  renderTurnusButtons();

  // Restore Campuses
  try {
    const savedCampuses = JSON.parse(localStorage.getItem(STORAGE_KEYS.CAMPUSES) || '[]');
    if (Array.isArray(savedCampuses)) {
      document.querySelectorAll('input[name="campus"]').forEach(chk => {
        chk.checked = savedCampuses.includes(chk.value);
      });
    }
  } catch(e) {}
  updateCampusChips();

  document.querySelectorAll('input[name="campus"]').forEach(chk => {
    chk.addEventListener('change', () => {
      const selected = Array.from(document.querySelectorAll('input[name="campus"]:checked')).map(c => c.value);
      localStorage.setItem(STORAGE_KEYS.CAMPUSES, JSON.stringify(selected));
      updateCampusChips();
      refreshModules();
    });
  });

  // Restore Campus Strict
  const savedStrict = localStorage.getItem(STORAGE_KEYS.CAMPUS_STRICT);
  const strictCheck = document.getElementById('filter-campus-strict');
  if (savedStrict !== null && strictCheck) {
    strictCheck.checked = savedStrict === 'true';
  }
  if (strictCheck) {
    strictCheck.addEventListener('change', () => {
      localStorage.setItem(STORAGE_KEYS.CAMPUS_STRICT, strictCheck.checked);
      refreshModules();
    });
  }

  // Restore Limitation
  const savedLimit = localStorage.getItem(STORAGE_KEYS.LIMITATION) || 'ja';
  setLimitationFilter(savedLimit, false);

  // Restore Languages
  const savedLangDE = localStorage.getItem(STORAGE_KEYS.LANG_DE);
  const langDECheck = document.getElementById('filter-lang-de');
  if (savedLangDE !== null && langDECheck) {
    langDECheck.checked = savedLangDE === 'true';
  }
  if (langDECheck) {
    langDECheck.addEventListener('change', () => {
      localStorage.setItem(STORAGE_KEYS.LANG_DE, langDECheck.checked);
      updateLanguageChips();
      refreshModules();
    });
  }

  const savedLangEN = localStorage.getItem(STORAGE_KEYS.LANG_EN);
  const langENCheck = document.getElementById('filter-lang-en');
  if (savedLangEN !== null && langENCheck) {
    langENCheck.checked = savedLangEN === 'true';
  }
  if (langENCheck) {
    langENCheck.addEventListener('change', () => {
      localStorage.setItem(STORAGE_KEYS.LANG_EN, langENCheck.checked);
      updateLanguageChips();
      refreshModules();
    });
  }
  updateLanguageChips();

  // Restore FÜS Filter
  let savedFUES = localStorage.getItem(STORAGE_KEYS.FUES);
  if (!savedFUES) {
    if (localStorage.getItem(STORAGE_KEYS.ONLY_FUES) === 'true') {
      savedFUES = 'nur';
    } else {
      savedFUES = 'inkl';
    }
  }
  setFUESFilter(savedFUES, false);

  // Restore Whitelisted Instructors
  try {
    const savedProfs = JSON.parse(localStorage.getItem(STORAGE_KEYS.INSTRUCTORS) || '[]');
    if (Array.isArray(savedProfs)) {
      selectedInstructors = savedProfs;
    }
  } catch(e) {
    selectedInstructors = [];
  }
  renderSelectedProfChips();

  // Keyboard navigation on prof search input
  const profSearch = document.getElementById('prof-search-input');
  if (profSearch) {
    profSearch.addEventListener('keydown', function(evt) {
      const items = getVisibleProfItems();
      const isMenuVisible = document.getElementById('prof-dropdown-menu')?.style.display !== 'none' && items.length > 0;

      if (evt.key === 'ArrowDown') {
        if (!isMenuVisible) return;
        evt.preventDefault();
        if (highlightedProfIndex < items.length - 1) {
          highlightedProfIndex++;
        } else {
          highlightedProfIndex = 0;
        }
        updateHighlightedProf(items);
      } else if (evt.key === 'ArrowUp') {
        if (!isMenuVisible) return;
        evt.preventDefault();
        if (highlightedProfIndex > 0) {
          highlightedProfIndex--;
        } else if (highlightedProfIndex === 0) {
          highlightedProfIndex = -1;
        } else {
          highlightedProfIndex = items.length - 1;
        }
        updateHighlightedProf(items);
      } else if (evt.key === 'Enter') {
        if (isMenuVisible && highlightedProfIndex >= 0 && items[highlightedProfIndex]) {
          evt.preventDefault();
          const profName = items[highlightedProfIndex].dataset.name;
          window.addInstructor(profName);
        }
      } else if (evt.key === 'Escape') {
        hideProfDropdown();
      }
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
      refreshModules();
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
      refreshModules();
    });
  }

  // Restore Min Credits Slider
  const savedCredits = localStorage.getItem(STORAGE_KEYS.MIN_CREDITS);
  const creditsSlider = document.getElementById('filter-min-credits');
  if (savedCredits !== null && creditsSlider) {
    creditsSlider.value = savedCredits;
    updateCreditsLabel(savedCredits);
  }
  if (creditsSlider) {
    creditsSlider.addEventListener('change', () => {
      refreshModules();
    });
  }

  // View mode is permanently table
  const viewInput = document.getElementById('filter-view');
  if (viewInput) viewInput.value = 'table';

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
