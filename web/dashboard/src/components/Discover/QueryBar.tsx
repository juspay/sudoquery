import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Box,
  ButtonBase,
  ClickAwayListener,
  IconButton,
  InputBase,
  Menu,
  MenuItem,
  Paper,
  Popper,
  Tooltip,
  Typography,
} from '@mui/material';
import { ChevronDown as ChevronDownIcon, Search as SearchIcon } from 'lucide-react';
import { doesKueryExpressionHaveLuceneSyntaxError } from '../../vendor/kuery';
import {
  applySuggestion,
  conjunctionSuggestions,
  cursorContext,
  fieldSuggestions,
  operatorSuggestions,
  valueLookup,
  valueSuggestions,
  type Suggestion,
  type SuggestionKind,
} from '../../discover/suggest';
import type { FieldDef, QueryLanguage } from '../../discover/types';
import {
  colorBlue,
  colorBluePale,
  colorCream,
  colorCream2,
  colorInk40,
  colorInk60,
  colorRose,
  radiusInput,
} from '../../theme/tokens';
import { HAIRLINE, monoSx } from './styles';

const LANGUAGE_LABEL: Record<QueryLanguage, string> = { dql: 'DQL', lucene: 'Lucene' };

const KIND_LABEL: Record<SuggestionKind, string> = {
  field: 'field',
  operator: 'operator',
  value: 'value',
  conjunction: 'conjunction',
};

const VALUE_LOOKUP_DELAY_MS = 150;

interface QueryBarProps {
  query: string;
  language: QueryLanguage;
  fields: FieldDef[];
  /** Why the submitted query could not be run. */
  error?: string | null;
  placeholder?: string;
  onSubmit: (query: string) => void;
  /** Omit to fix the language to DQL. */
  onLanguageChange?: (language: QueryLanguage) => void;
  /** Values of `field` starting with `prefix`, for autocomplete. */
  lookupValues?: (field: string, prefix: string) => Promise<string[]>;
}

export function QueryBar({
  query,
  language,
  fields,
  error,
  placeholder,
  onSubmit,
  onLanguageChange,
  lookupValues,
}: QueryBarProps) {
  const [edit, setEdit] = useState({ source: query, draft: query });
  const [caret, setCaret] = useState(query.length);
  const [open, setOpen] = useState(false);
  const [highlighted, setHighlighted] = useState(-1);
  const [values, setValues] = useState<{ key: string; values: string[] } | null>(null);
  const [languageAnchor, setLanguageAnchor] = useState<HTMLElement | null>(null);
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // A query arriving from outside (URL, a clicked filter) replaces the draft.
  if (edit.source !== query) {
    setEdit({ source: query, draft: query });
  }
  const draft = edit.draft;
  const setDraft = (next: string) => setEdit({ source: query, draft: next });
  // An error is about the submitted query; it stops applying once that is edited.
  const shownError = error && draft.trim() === query.trim() ? error : null;

  const context = useMemo(
    () => (language === 'dql' && open ? cursorContext(draft, caret) : null),
    [language, open, draft, caret],
  );
  const lookup = useMemo(() => (context ? valueLookup(context) : null), [context]);
  const lookupField = lookupValues ? (lookup?.field ?? null) : null;
  const lookupPrefix = lookup?.prefix ?? '';
  const lookupKey = lookupField === null ? null : `${lookupField}\n${lookupPrefix}`;

  useEffect(() => {
    if (!lookupValues || lookupField === null) return;
    const key = `${lookupField}\n${lookupPrefix}`;
    let stale = false;
    const timer = window.setTimeout(() => {
      lookupValues(lookupField, lookupPrefix).then(
        (found) => {
          if (!stale) setValues({ key, values: found });
        },
        () => {
          if (!stale) setValues({ key, values: [] });
        },
      );
    }, VALUE_LOOKUP_DELAY_MS);
    return () => {
      stale = true;
      window.clearTimeout(timer);
    };
  }, [lookupField, lookupPrefix, lookupValues]);

  const suggestions = useMemo<Suggestion[]>(() => {
    if (!context) return [];
    const found = lookupKey !== null && values?.key === lookupKey ? values.values : [];
    const field = fields.find((candidate) => candidate.name === context.fieldName);
    return [
      ...valueSuggestions(context, found, field),
      ...fieldSuggestions(context, fields),
      ...operatorSuggestions(context, fields),
      ...conjunctionSuggestions(context),
    ];
  }, [context, fields, lookupKey, values]);

  const looksLikeLucene = useMemo(
    () => language === 'dql' && query.trim() !== '' && doesKueryExpressionHaveLuceneSyntaxError(query),
    [language, query],
  );

  const submit = () => {
    setOpen(false);
    setHighlighted(-1);
    onSubmit(draft.trim());
  };

  const accept = (suggestion: Suggestion) => {
    const next = applySuggestion(draft, suggestion);
    setDraft(next.query);
    setCaret(next.caret);
    setHighlighted(-1);
    setOpen(true);
    // Put the caret after the inserted text once React has written the value.
    requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.setSelectionRange(next.caret, next.caret);
    });
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    const showing = open && suggestions.length > 0;
    switch (event.key) {
      case 'ArrowDown':
        if (!showing) return;
        event.preventDefault();
        setHighlighted((index) => (index + 1) % suggestions.length);
        return;
      case 'ArrowUp':
        if (!showing) return;
        event.preventDefault();
        setHighlighted((index) => (index <= 0 ? suggestions.length - 1 : index - 1));
        return;
      case 'Tab':
        if (!showing) return;
        event.preventDefault();
        accept(suggestions[Math.max(highlighted, 0)]);
        return;
      case 'Enter':
        event.preventDefault();
        if (showing && highlighted >= 0) {
          accept(suggestions[highlighted]);
        } else {
          submit();
        }
        return;
      case 'Escape':
        setOpen(false);
        setHighlighted(-1);
        return;
    }
  };

  const syncCaret = () => {
    const input = inputRef.current;
    if (input) setCaret(input.selectionStart ?? input.value.length);
  };

  return (
    <Box sx={{ flex: 1, minWidth: 0 }}>
      <ClickAwayListener onClickAway={() => setOpen(false)}>
        <Box>
          <Box
            ref={setAnchor}
            sx={{
              display: 'flex',
              alignItems: 'center',
              height: 40,
              bgcolor: '#FFFFFF',
              border: `1.5px solid ${shownError ? colorRose : colorCream2}`,
              borderRadius: `${radiusInput}px`,
              transition: 'border-color 0.15s',
              '&:focus-within': { borderColor: shownError ? colorRose : colorBlue },
            }}
          >
            {onLanguageChange ? (
              <ButtonBase
                onClick={(event) => setLanguageAnchor(event.currentTarget)}
                aria-label="Query language"
                sx={{
                  alignSelf: 'stretch',
                  px: 1.25,
                  gap: 0.5,
                  borderRight: HAIRLINE,
                  fontSize: '12px',
                  fontWeight: 600,
                  color: colorInk60,
                  '&:hover': { bgcolor: colorCream },
                }}
              >
                {LANGUAGE_LABEL[language]}
                <ChevronDownIcon size={13} />
              </ButtonBase>
            ) : null}
            <InputBase
              inputRef={inputRef}
              value={draft}
              placeholder={
                placeholder ??
                (language === 'dql'
                  ? 'Search events — e.g. name:checkout_viewed and properties.plan:pro'
                  : 'Search events with Lucene — e.g. name:checkout_viewed AND properties.plan:pro')
              }
              onChange={(event) => {
                setDraft(event.target.value);
                setCaret(event.target.selectionStart ?? event.target.value.length);
                setHighlighted(-1);
                setOpen(true);
              }}
              onFocus={() => setOpen(true)}
              onClick={syncCaret}
              onKeyUp={syncCaret}
              onKeyDown={handleKeyDown}
              inputProps={{ 'aria-label': 'Search query', spellCheck: false, autoComplete: 'off' }}
              sx={{ flex: 1, px: 1.25, ...monoSx, fontSize: '13px' }}
            />
            <Tooltip title="Run search (Enter)">
              <IconButton onClick={submit} size="small" sx={{ mr: 0.5, color: colorBlue }}>
                <SearchIcon size={16} />
              </IconButton>
            </Tooltip>
          </Box>

          <Popper
            open={open && suggestions.length > 0}
            anchorEl={anchor}
            placement="bottom-start"
            sx={{ zIndex: 1300 }}
          >
            <Paper
              elevation={0}
              sx={{
                mt: 0.5,
                width: anchor?.clientWidth,
                maxHeight: 320,
                overflowY: 'auto',
                border: HAIRLINE,
                boxShadow: '0 8px 24px rgba(24,22,15,0.10)',
              }}
            >
              {suggestions.map((suggestion, index) => (
                <Box
                  key={`${suggestion.kind}:${suggestion.label}`}
                  role="option"
                  aria-selected={index === highlighted}
                  // Keep focus in the input while choosing.
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => accept(suggestion)}
                  onMouseEnter={() => setHighlighted(index)}
                  sx={{
                    display: 'flex',
                    alignItems: 'baseline',
                    gap: 1.5,
                    px: 1.5,
                    py: 0.75,
                    cursor: 'pointer',
                    bgcolor: index === highlighted ? colorBluePale : 'transparent',
                  }}
                >
                  <Typography
                    sx={{ width: 78, flexShrink: 0, fontSize: '11px', color: colorInk40 }}
                  >
                    {KIND_LABEL[suggestion.kind]}
                  </Typography>
                  <Typography
                    sx={{ ...monoSx, color: 'text.primary', overflowWrap: 'anywhere' }}
                  >
                    {suggestion.label}
                  </Typography>
                  {suggestion.detail ? (
                    <Typography sx={{ ml: 'auto', fontSize: '11px', color: colorInk40 }}>
                      {suggestion.detail}
                    </Typography>
                  ) : null}
                </Box>
              ))}
            </Paper>
          </Popper>
        </Box>
      </ClickAwayListener>

      {shownError ? (
        <Typography role="alert" sx={{ mt: 0.5, fontSize: '12px', color: colorRose }}>
          {shownError}
        </Typography>
      ) : null}
      {!shownError && looksLikeLucene && onLanguageChange ? (
        <Typography sx={{ mt: 0.5, fontSize: '12px', color: colorInk60 }}>
          This looks like Lucene syntax, which DQL reads differently.{' '}
          <Box
            component="button"
            type="button"
            onClick={() => onLanguageChange('lucene')}
            sx={{
              p: 0,
              border: 'none',
              background: 'none',
              font: 'inherit',
              color: colorBlue,
              cursor: 'pointer',
              textDecoration: 'underline',
            }}
          >
            Switch to Lucene
          </Box>
        </Typography>
      ) : null}

      <Menu
        anchorEl={languageAnchor}
        open={languageAnchor !== null}
        onClose={() => setLanguageAnchor(null)}
      >
        {(Object.keys(LANGUAGE_LABEL) as QueryLanguage[]).map((option) => (
          <MenuItem
            key={option}
            selected={option === language}
            onClick={() => {
              setLanguageAnchor(null);
              onLanguageChange?.(option);
            }}
            sx={{ fontSize: '13px' }}
          >
            {LANGUAGE_LABEL[option]}
          </MenuItem>
        ))}
      </Menu>
    </Box>
  );
}
