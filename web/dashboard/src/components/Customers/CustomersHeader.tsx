import { Link as RouterLink } from 'react-router-dom';
import { Tab, Tabs } from '@mui/material';
import { colorCream2 } from '../../theme/tokens';

type CustomersTab = 'customers' | 'segments' | 'setup';

const TAB_PATH: Record<CustomersTab, string> = {
  customers: '/app/customers',
  segments: '/app/customers/segments',
  setup: '/app/customers/setup',
};

const TAB_LABEL: Record<CustomersTab, string> = {
  customers: 'Customers',
  segments: 'Segments',
  setup: 'Setup',
};

/** The Customers / Segments / Setup tabs. */
export function CustomersHeader({ tab }: { tab: CustomersTab }) {
  return (
    <Tabs
        value={tab}
        sx={{
          minHeight: 34,
          borderBottom: `1px solid ${colorCream2}`,
          '& .MuiTab-root': { minHeight: 34, py: 0, fontSize: '13px', fontWeight: 600 },
        }}
      >
        {(Object.keys(TAB_PATH) as CustomersTab[]).map((option) => (
          <Tab
            key={option}
            value={option}
            label={TAB_LABEL[option]}
            component={RouterLink}
            to={TAB_PATH[option]}
          />
        ))}
    </Tabs>
  );
}
