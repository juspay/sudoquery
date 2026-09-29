import { memo, useState, useCallback, useRef, useEffect } from 'react';
import { Autocomplete, TextField, CircularProgress, Popper, Box, Typography } from '@mui/material';

interface SearchableDropdownProps {
  /** Currently selected value */
  value: string;
  /** Callback when value changes (selection) */
  onChange: (value: string) => void;
  /** Function to get dropdown options based on search value */
  getQuery: (searchValue: string) => Promise<string[]>;
  /** Label/description for the input */
  label?: string;
  /** Placeholder text */
  placeholder?: string;
  /** Full width */
  fullWidth?: boolean;
  /** Height of the input */
  height?: number;
}

export const SearchableDropdown = memo(({
  value,
  onChange,
  getQuery,
  label,
  placeholder = 'Search...',
  fullWidth = true,
  height = 32,
}: SearchableDropdownProps) => {
  const [options, setOptions] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [inputValue, setInputValue] = useState(value);
  const debounceRef = useRef<NodeJS.Timeout>();

  // Sync inputValue with value prop when value changes
  useEffect(() => {
    setInputValue(value);
  }, [value]);

  const handleInputChange = useCallback((newValue: string) => {
    setInputValue(newValue);

    // Debounce the query
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    debounceRef.current = setTimeout(async () => {
      setLoading(true);
      try {
        const newOptions = await getQuery(newValue);
        setOptions(newOptions);
      } catch (error) {
        console.error('Failed to load dropdown options:', error);
        setOptions([]);
      } finally {
        setLoading(false);
      }
    }, 300);
  }, [getQuery]);

  const handleChange = useCallback((newValue: string) => {
    onChange(newValue);
    // Sync input value with selected value
    setInputValue(newValue);
  }, [onChange]);

  return (
    <Box>
      <Autocomplete
        value={value || ''}
        onChange={(e, newValue) => handleChange(newValue || '')}
        options={options}
        loading={loading}
        size="small"
        fullWidth={fullWidth}
        freeSolo
        includeInputInList
        disablePortal
        PopperComponent={(props) => <Popper {...props} placement="bottom-start" />}
        inputValue={inputValue}
        onInputChange={(e, newInputValue, reason) => {
          // Only update input value on user input, not when selecting from dropdown
          if (reason !== 'reset') {
            handleInputChange(newInputValue);
          }
        }}
        renderInput={(params) => (
          <TextField
            {...params}
            placeholder={loading ? 'Loading...' : placeholder}
            InputProps={{
              ...params.InputProps,
              endAdornment: (
                <>
                  {loading ? <CircularProgress size={16} /> : null}
                  {params.InputProps.endAdornment}
                </>
              ),
            }}
            sx={{
              height,
              fontSize: '0.875rem',
              '& .MuiOutlinedInput-root': {
                height,
              },
              '& .MuiOutlinedInput-input': {
                py: 0.5,
                height,
              },
            }}
          />
        )}
        sx={{
          fontSize: '0.875rem',
        }}
      />
    </Box>
  );
});

SearchableDropdown.displayName = 'SearchableDropdown';