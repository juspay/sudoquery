import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { Box, Button, Container, Divider, IconButton, Paper, TextField, Typography } from '@mui/material';
import { ArrowLeft, CheckCircle2, Minus, Pencil, Plus, Send, ShoppingBag, Trash2 } from 'lucide-react';
import { DEMO_CONFIG } from '../config/api';
import { useOrganization } from '../contexts/OrganizationContext';
import { useProject } from '../contexts/ProjectContext';
import { CURRENCY, PRODUCTS, formatPrice, type Product } from '../demo/catalog';
import { isUuid, loadDestination, saveDestination, type Destination } from '../demo/destination';
import {
  flushDemoEvents,
  identifyDemoCustomer,
  initDemoAnalytics,
  trackDemoEvent,
  useTrackedEvents,
} from '../demo/analytics';
import {
  colorBlue,
  colorCream,
  colorCream3,
  colorInk,
  colorInk40,
  colorInk60,
  colorInk80,
  fontFamilyDisplay,
  fontFamilyMono,
  radiusCard,
  topbarHeight,
} from '../theme/tokens';

/** Product id → quantity. */
type Cart = Record<string, number>;
type CartLine = { product: Product; quantity: number };
type Step = 'shop' | 'checkout' | 'done';

function cartLines(cart: Cart): CartLine[] {
  return PRODUCTS.filter((product) => cart[product.id]).map((product) => ({
    product,
    quantity: cart[product.id],
  }));
}

function cartTotals(cart: Cart) {
  const lines = cartLines(cart);
  return {
    item_count: lines.reduce((count, line) => count + line.quantity, 0),
    cart_value: lines.reduce((sum, line) => sum + line.quantity * line.product.price, 0),
  };
}

function lineItems(cart: Cart) {
  return cartLines(cart).map(({ product, quantity }) => ({
    product_id: product.id,
    quantity,
    price: product.price,
  }));
}

export default function DemoStorePage() {
  const [destination, setDestination] = useState(loadDestination);

  const start = (next: Destination) => {
    saveDestination(next);
    setDestination(next);
  };

  return destination ? <DemoStore destination={destination} /> : <DestinationSetup onStart={start} />;
}

function DemoStore({ destination }: { destination: Destination }) {
  const [cart, setCart] = useState<Cart>({});
  const [step, setStep] = useState<Step>('shop');
  const [email, setEmail] = useState('');
  const [orderId, setOrderId] = useState<string | null>(null);
  const viewTracked = useRef(false);

  useEffect(() => {
    // StrictMode runs effects twice in development; count the view once.
    if (viewTracked.current) return;
    viewTracked.current = true;
    initDemoAnalytics(destination);
    trackDemoEvent('product_list_viewed', { product_count: PRODUCTS.length });
  }, [destination]);

  const setQuantity = (product: Product, quantity: number) => {
    const previous = cart[product.id] ?? 0;
    const next = { ...cart, [product.id]: quantity };
    if (quantity === 0) delete next[product.id];
    setCart(next);

    trackDemoEvent(quantity > previous ? 'product_added_to_cart' : 'product_removed_from_cart', {
      product_id: product.id,
      product_name: product.name,
      category: product.category,
      price: product.price,
      quantity: Math.abs(quantity - previous),
      ...cartTotals(next),
    });
  };

  const startCheckout = () => {
    setStep('checkout');
    trackDemoEvent('checkout_started', {
      ...cartTotals(cart),
      currency: CURRENCY,
      products: lineItems(cart),
    });
  };

  const placeOrder = () => {
    const id = `ORD-${crypto.randomUUID().slice(0, 8).toUpperCase()}`;
    const customer = email.trim();
    if (customer) identifyDemoCustomer(customer);

    const { item_count, cart_value } = cartTotals(cart);
    trackDemoEvent('order_completed', {
      order_id: id,
      revenue: cart_value,
      item_count,
      currency: CURRENCY,
      products: lineItems(cart),
    });
    // Send the order right away rather than on the next timer tick.
    void flushDemoEvents();

    setOrderId(id);
    setCart({});
    setStep('done');
  };

  const lines = cartLines(cart);
  const { item_count, cart_value } = cartTotals(cart);

  return (
    <Box sx={{ minHeight: '100vh', bgcolor: colorCream }}>
      <StoreHeader itemCount={item_count} />
      <Container maxWidth="lg" sx={{ py: { xs: 3, md: 5 } }}>
        <Box
          sx={{
            display: 'grid',
            gap: 3,
            gridTemplateColumns: { xs: 'minmax(0, 1fr)', md: 'minmax(0, 1fr) 360px' },
            alignItems: 'start',
          }}
        >
          <Box component="main">
            {step === 'shop' && (
              <ProductGrid cart={cart} onAdd={(product) => setQuantity(product, (cart[product.id] ?? 0) + 1)} />
            )}
            {step === 'checkout' && (
              <Checkout
                lines={lines}
                total={cart_value}
                email={email}
                onEmailChange={setEmail}
                onBack={() => setStep('shop')}
                onPlaceOrder={placeOrder}
              />
            )}
            {step === 'done' && orderId && (
              <OrderConfirmation orderId={orderId} onContinue={() => setStep('shop')} />
            )}
          </Box>
          <Box
            component="aside"
            sx={{ display: 'flex', flexDirection: 'column', gap: 3, position: { md: 'sticky' }, top: { md: 24 } }}
          >
            {step === 'shop' && (
              <CartPanel lines={lines} total={cart_value} onQuantityChange={setQuantity} onCheckout={startCheckout} />
            )}
            <DestinationPanel destination={destination} />
            <EventLog />
          </Box>
        </Box>
      </Container>
    </Box>
  );
}

function StoreHeader({ itemCount }: { itemCount: number }) {
  return (
    <Box component="header" sx={{ bgcolor: '#FFFFFF', borderBottom: `1px solid ${colorCream3}` }}>
      <Container
        maxWidth="lg"
        sx={{ height: topbarHeight, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
          <Typography
            sx={{
              fontFamily: fontFamilyDisplay,
              fontSize: '18px',
              fontWeight: 700,
              color: colorInk,
              letterSpacing: '-0.5px',
              '& em': { color: colorBlue, fontStyle: 'italic' },
            }}
          >
            Sudo<em>query</em>
          </Typography>
          <Divider orientation="vertical" flexItem sx={{ my: 2 }} />
          <Typography variant="body2" sx={{ color: colorInk60 }}>
            Demo store
          </Typography>
        </Box>
        {/* On narrow screens the cart sits below the products. */}
        <Box
          component="a"
          href="#cart"
          sx={{ display: 'flex', alignItems: 'center', gap: 1, color: colorInk60, textDecoration: 'none' }}
        >
          <ShoppingBag size={18} />
          <Typography variant="body2">
            {itemCount} {itemCount === 1 ? 'item' : 'items'}
          </Typography>
        </Box>
      </Container>
    </Box>
  );
}

function ProductGrid({ cart, onAdd }: { cart: Cart; onAdd: (product: Product) => void }) {
  return (
    <>
      <Typography variant="h2" sx={{ mb: 0.5 }}>
        Shop
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 3 }}>
        Add a few things to your cart and check out. Each step sends an event to Sudoquery.
      </Typography>
      <Box
        sx={{
          display: 'grid',
          gap: 2,
          gridTemplateColumns: {
            xs: 'minmax(0, 1fr)',
            sm: 'repeat(2, minmax(0, 1fr))',
            lg: 'repeat(3, minmax(0, 1fr))',
          },
        }}
      >
        {PRODUCTS.map((product) => (
          <ProductCard
            key={product.id}
            product={product}
            quantity={cart[product.id] ?? 0}
            onAdd={() => onAdd(product)}
          />
        ))}
      </Box>
    </>
  );
}

function ProductCard({ product, quantity, onAdd }: { product: Product; quantity: number; onAdd: () => void }) {
  const Icon = product.icon;
  return (
    <Paper
      variant="outlined"
      sx={{
        borderRadius: `${radiusCard}px`,
        borderColor: colorCream3,
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <Box sx={{ height: { xs: 96, sm: 132 }, bgcolor: product.tint, color: colorInk80, display: 'grid', placeItems: 'center' }}>
        <Icon size={44} strokeWidth={1.5} />
      </Box>
      <Box sx={{ p: 2, flex: 1, display: 'flex', flexDirection: 'column' }}>
        <Typography
          variant="caption"
          sx={{ color: colorInk40, textTransform: 'uppercase', letterSpacing: '0.08em' }}
        >
          {product.category}
        </Typography>
        <Typography sx={{ fontWeight: 600, color: colorInk, mb: 0.5 }}>{product.name}</Typography>
        <Box sx={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', mb: 2 }}>
          <Typography sx={{ color: colorInk60 }}>{formatPrice(product.price)}</Typography>
          {quantity > 0 && (
            <Typography variant="caption" sx={{ color: colorBlue, fontWeight: 600 }}>
              {quantity} in cart
            </Typography>
          )}
        </Box>
        <Button variant="outlined" size="small" startIcon={<Plus size={16} />} onClick={onAdd} sx={{ mt: 'auto' }}>
          Add to cart
        </Button>
      </Box>
    </Paper>
  );
}

function CartPanel({
  lines,
  total,
  onQuantityChange,
  onCheckout,
}: {
  lines: CartLine[];
  total: number;
  onQuantityChange: (product: Product, quantity: number) => void;
  onCheckout: () => void;
}) {
  return (
    <Panel id="cart" title="Cart">
      {lines.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          Your cart is empty.
        </Typography>
      ) : (
        <Box component="ul" sx={{ listStyle: 'none', m: 0, p: 0, display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          {lines.map(({ product, quantity }) => (
            <Box component="li" key={product.id} sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
              <Box sx={{ flex: 1, minWidth: 0 }}>
                <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                  {product.name}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {formatPrice(product.price * quantity)}
                </Typography>
              </Box>
              <IconButton
                size="small"
                aria-label={`Remove one ${product.name}`}
                onClick={() => onQuantityChange(product, quantity - 1)}
              >
                <Minus size={14} />
              </IconButton>
              <Typography variant="body2" sx={{ minWidth: 20, textAlign: 'center' }}>
                {quantity}
              </Typography>
              <IconButton
                size="small"
                aria-label={`Add one ${product.name}`}
                onClick={() => onQuantityChange(product, quantity + 1)}
              >
                <Plus size={14} />
              </IconButton>
              <IconButton
                size="small"
                aria-label={`Remove ${product.name} from cart`}
                onClick={() => onQuantityChange(product, 0)}
              >
                <Trash2 size={14} />
              </IconButton>
            </Box>
          ))}
        </Box>
      )}
      <Divider />
      <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
        <Typography variant="body2" color="text.secondary">
          Subtotal
        </Typography>
        <Typography variant="body2" sx={{ fontWeight: 600 }}>
          {formatPrice(total)}
        </Typography>
      </Box>
      <Button variant="contained" fullWidth disabled={lines.length === 0} onClick={onCheckout}>
        Checkout
      </Button>
    </Panel>
  );
}

function Checkout({
  lines,
  total,
  email,
  onEmailChange,
  onBack,
  onPlaceOrder,
}: {
  lines: CartLine[];
  total: number;
  email: string;
  onEmailChange: (email: string) => void;
  onBack: () => void;
  onPlaceOrder: () => void;
}) {
  const submit = (event: FormEvent) => {
    event.preventDefault();
    onPlaceOrder();
  };

  return (
    <Box sx={{ maxWidth: 560 }}>
      <Button size="small" startIcon={<ArrowLeft size={16} />} onClick={onBack} sx={{ mb: 2 }}>
        Back to shop
      </Button>
      <Typography variant="h2" sx={{ mb: 3 }}>
        Checkout
      </Typography>
      <Panel title="Order summary">
        <Box component="ul" sx={{ listStyle: 'none', m: 0, p: 0, display: 'flex', flexDirection: 'column', gap: 1 }}>
          {lines.map(({ product, quantity }) => (
            <Box component="li" key={product.id} sx={{ display: 'flex', justifyContent: 'space-between', gap: 2 }}>
              <Typography variant="body2">
                {product.name} × {quantity}
              </Typography>
              <Typography variant="body2">{formatPrice(product.price * quantity)}</Typography>
            </Box>
          ))}
        </Box>
        <Divider />
        <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
          <Typography sx={{ fontWeight: 600 }}>Total</Typography>
          <Typography sx={{ fontWeight: 600 }}>{formatPrice(total)}</Typography>
        </Box>
        <Box component="form" onSubmit={submit} sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <TextField
            label="Email (optional)"
            type="email"
            size="small"
            value={email}
            onChange={(event) => onEmailChange(event.target.value)}
            helperText="Sent as the customer's user ID (actor_id)."
          />
          <Button type="submit" variant="contained" size="large">
            Place order · {formatPrice(total)}
          </Button>
        </Box>
      </Panel>
    </Box>
  );
}

function OrderConfirmation({ orderId, onContinue }: { orderId: string; onContinue: () => void }) {
  return (
    <Paper
      variant="outlined"
      sx={{
        maxWidth: 560,
        p: { xs: 3, sm: 5 },
        borderRadius: `${radiusCard}px`,
        borderColor: colorCream3,
        textAlign: 'center',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 1.5,
      }}
    >
      <CheckCircle2 size={48} color={colorBlue} strokeWidth={1.5} />
      <Typography variant="h2">Order placed</Typography>
      <Typography color="text.secondary">
        Order <strong>{orderId}</strong> went to Sudoquery as an <Code>order_completed</Code> event.
      </Typography>
      <Button variant="contained" onClick={onContinue} sx={{ mt: 1.5 }}>
        Continue shopping
      </Button>
    </Paper>
  );
}

function EventLog() {
  const events = useTrackedEvents();
  const [sending, setSending] = useState(false);

  const sendNow = async () => {
    setSending(true);
    try {
      await flushDemoEvents();
    } finally {
      setSending(false);
    }
  };

  return (
    <Panel
      title="Events"
      action={
        <Button size="small" startIcon={<Send size={14} />} onClick={sendNow} disabled={sending}>
          Send now
        </Button>
      }
    >
      <Typography variant="caption" color="text.secondary">
        Events go out in batches of 5 or every 3 seconds. The requests show up in your browser's Network tab.
      </Typography>
      <Box
        component="ol"
        sx={{ listStyle: 'none', m: 0, p: 0, maxHeight: 420, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 1 }}
      >
        {events.map((event) => (
          <Box
            component="li"
            key={event.key}
            sx={{ p: 1.25, bgcolor: colorCream, border: `1px solid ${colorCream3}`, borderRadius: `${radiusCard}px` }}
          >
            <Box sx={{ display: 'flex', justifyContent: 'space-between', gap: 1 }}>
              <Typography sx={{ fontFamily: fontFamilyMono, fontSize: 12, fontWeight: 600, color: colorInk }}>
                {event.name}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                {event.at.toLocaleTimeString()}
              </Typography>
            </Box>
            <Box
              component="pre"
              sx={{
                m: 0,
                mt: 0.5,
                fontFamily: fontFamilyMono,
                fontSize: 11,
                color: colorInk60,
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
              }}
            >
              {JSON.stringify(event.properties, null, 2)}
            </Box>
          </Box>
        ))}
      </Box>
    </Panel>
  );
}

function DestinationPanel({ destination }: { destination: Destination }) {
  const [editing, setEditing] = useState(false);

  const apply = async (next: Destination) => {
    saveDestination(next);
    // The SDK can't be re-pointed once it has started. Send what's queued to
    // the old destination, then reload to start over with the new one.
    await flushDemoEvents();
    window.location.reload();
  };

  return (
    <Panel
      title="Destination"
      action={
        !editing && (
          <Button size="small" startIcon={<Pencil size={14} />} onClick={() => setEditing(true)}>
            Change
          </Button>
        )
      }
    >
      {editing ? (
        <DestinationForm
          initial={destination}
          submitLabel="Apply and reload"
          onSubmit={apply}
          onCancel={() => setEditing(false)}
        />
      ) : (
        <Box
          component="dl"
          sx={{
            m: 0,
            display: 'grid',
            gridTemplateColumns: 'auto minmax(0, 1fr)',
            columnGap: 1.5,
            rowGap: 0.5,
            fontSize: 12,
            '& dt': { color: colorInk40 },
            '& dd': { m: 0, fontFamily: fontFamilyMono, color: colorInk80, wordBreak: 'break-all' },
          }}
        >
          <dt>Collector</dt>
          <dd>{DEMO_CONFIG.COLLECTOR_URL}</dd>
          <dt>Organization</dt>
          <dd>{destination.tenantId}</dd>
          <dt>Project</dt>
          <dd>{destination.workspaceId}</dd>
        </Box>
      )}
    </Panel>
  );
}

function DestinationSetup({ onStart }: { onStart: (destination: Destination) => void }) {
  return (
    <Box sx={{ minHeight: '100vh', bgcolor: colorCream, display: 'grid', placeItems: 'center', p: 3 }}>
      <Paper
        variant="outlined"
        sx={{ width: '100%', maxWidth: 480, p: { xs: 3, sm: 4 }, borderRadius: `${radiusCard}px`, borderColor: colorCream3 }}
      >
        <Typography variant="h3" sx={{ mb: 1 }}>
          Where should the events go?
        </Typography>
        <Typography color="text.secondary" sx={{ mb: 3 }}>
          The demo store files its events under a Sudoquery organization and project. You can change them later.
        </Typography>
        <DestinationForm submitLabel="Open the store" onSubmit={onStart} />
      </Paper>
    </Box>
  );
}

function DestinationForm({
  initial,
  submitLabel,
  onSubmit,
  onCancel,
}: {
  initial?: Destination;
  submitLabel: string;
  onSubmit: (destination: Destination) => void;
  onCancel?: () => void;
}) {
  const { currentOrganization } = useOrganization();
  // Projects are only loaded for the current organization, so this one
  // belongs to it.
  const { currentProject } = useProject();
  const [tenantId, setTenantId] = useState(initial?.tenantId ?? '');
  const [workspaceId, setWorkspaceId] = useState(initial?.workspaceId ?? '');

  const tenant = tenantId.trim();
  const workspace = workspaceId.trim().toLowerCase();
  const workspaceInvalid = workspace !== '' && !isUuid(workspace);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSubmit({ tenantId: tenant, workspaceId: workspace });
  };

  const idInputProps = { htmlInput: { spellCheck: false, autoComplete: 'off', style: { fontFamily: fontFamilyMono } } };

  return (
    <Box component="form" onSubmit={submit} sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      {currentOrganization && currentProject && (
        <Button
          variant="outlined"
          size="small"
          onClick={() => {
            setTenantId(currentOrganization.id);
            setWorkspaceId(currentProject.id);
          }}
          sx={{ alignSelf: 'flex-start', textTransform: 'none' }}
        >
          Use {currentOrganization.name} / {currentProject.name}
        </Button>
      )}
      <TextField
        label="Organization ID"
        size="small"
        required
        value={tenantId}
        onChange={(event) => setTenantId(event.target.value)}
        helperText="Sent as the tenant (org_id)."
        slotProps={idInputProps}
      />
      <TextField
        label="Project ID"
        size="small"
        required
        value={workspaceId}
        onChange={(event) => setWorkspaceId(event.target.value)}
        error={workspaceInvalid}
        helperText={workspaceInvalid ? 'Must be a project UUID.' : 'Sent as the workspace (proj_id).'}
        slotProps={idInputProps}
      />
      <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1 }}>
        {onCancel && <Button onClick={onCancel}>Cancel</Button>}
        <Button type="submit" variant="contained" disabled={!tenant || !workspace || workspaceInvalid}>
          {submitLabel}
        </Button>
      </Box>
    </Box>
  );
}

function Panel({
  id,
  title,
  action,
  children,
}: {
  id?: string;
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Paper
      id={id}
      variant="outlined"
      sx={{
        scrollMarginTop: 16,
        p: 2.5,
        borderRadius: `${radiusCard}px`,
        borderColor: colorCream3,
        display: 'flex',
        flexDirection: 'column',
        gap: 2,
      }}
    >
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <Typography variant="h4">{title}</Typography>
        {action}
      </Box>
      {children}
    </Paper>
  );
}

function Code({ children }: { children: ReactNode }) {
  return (
    <Box component="code" sx={{ fontFamily: fontFamilyMono, fontSize: '0.9em', color: colorInk80 }}>
      {children}
    </Box>
  );
}
