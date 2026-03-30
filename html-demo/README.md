# SudoQuery HTML Demo

A complete HTML demo showcasing the SudoQuery library using the compiled JavaScript from the `dist/` folder.

## Features

### 🎯 Core Functionality
- **User Management**: Set and remove users for event tracking
- **Group Management**: Set and remove groups for segmentation
- **Event Tracking**: Track various event types (page views, clicks, forms, purchases, errors)
- **Configuration**: Dynamically change batch size
- **Real-time Logging**: View tracked events in real-time
- **Auto-flush**: Events automatically flush when batch size is reached

### 📊 Event Types Supported
- `page_view` - Track page visits
- `button_click` - Track button interactions
- `form_submit` - Track form submissions
- `purchase` - Track purchases
- `error` - Track errors

### 🎛️ Configuration Options
- Batch size: 2, 5, or 10 events
- User ID: Custom user identification
- Group ID: Custom group segmentation

## How to Run

### Option 1: Using Python Simple HTTP Server
```bash
cd html-demo
python3 -m http.server 8000
```
Then open: http://localhost:8000

### Option 2: Using Node.js HTTP Server
```bash
cd html-demo
npx http-server -p 8000
```
Then open: http://localhost:8000

### Option 3: Using PHP Built-in Server
```bash
cd html-demo
php -S localhost:8000
```
Then open: http://localhost:8000

### Option 4: Direct File Opening
Simply open `index.html` in your browser (may have CORS limitations).

## Usage Guide

### 1. Initialize Analytics
The analytics SDK is automatically initialized when the page loads. You'll see "Analytics initialized" in the event log.

### 2. Set User
1. Enter a user ID (e.g., `user_123`) in the input field
2. Click "Set User"
3. All subsequent events will include this user ID
4. The user badge will appear in the event log

### 3. Set Group
1. Enter a group ID (e.g., `premium_users`) in the input field
2. Click "Set Group"
3. All subsequent events will include this group ID
4. The group badge will appear in the event log

### 4. Track Events
Click any of the event buttons to track events:
- **Page View**: Tracks a page visit event
- **Button Click**: Tracks a button click event
- **Form Submit**: Tracks a form submission event
- **Purchase**: Tracks a purchase event
- **Error**: Tracks an error event

Each event will appear in the event log with:
- Timestamp
- Event name
- Description
- User badge (if user is set)
- Group badge (if group is set)

### 5. Change Batch Size
Click any batch size button to change how many events are batched before auto-flush:
- **Batch Size: 2** - Flushes after every 2 events
- **Batch Size: 5** - Flushes after every 5 events
- **Batch Size: 10** - Flushes after every 10 events

### 6. Remove User/Group
- Click "Remove User" to clear the current user ID (resets to null)
- Click "Remove Group" to clear the current group ID (resets to null)

## Event Log Format

```
[HH:MM:SS] event_name: description 👤 user_id 👥 group_id
```

- **Timestamp**: When the event was tracked
- **Event Name**: The type of event
- **Description**: Details about the event
- **User Badge**: Shows current user ID (if set)
- **Group Badge**: Shows current group ID (if set)

## Status Panel

The status panel shows:
- **Initialized**: Whether analytics SDK is initialized
- **Current User**: The currently set user ID or null
- **Current Group**: The currently set group ID or null
- **Batch Size**: The current batch size configuration
- **Events Tracked**: Total number of events tracked in this session

## Behind the Scenes

### How It Works

1. **Initialization**: On page load, `SudoQuery.init()` is called
2. **Event Tracking**: When you click a button, `SudoQuery.track()` is called
3. **Batching**: Events are stored in batches according to the batch size
4. **Auto-flush**: When batch reaches capacity, events are automatically flushed
5. **Page Unload**: Events are flushed using beacon API when page is closed

### Event Structure

Each tracked event has the following structure:

```javascript
{
  event: "event_name",
  properties: {
    // Event-specific properties
    // Super properties
    // Session details
  },
  user: "user_id" || null,
  group: "group_id" || null,
  at: 1704729600000  // Unix timestamp
}
```

## Browser Compatibility

- Chrome/Edge: ✅ Full support
- Firefox: ✅ Full support
- Safari: ✅ Full support
- Opera: ✅ Full support

## Technical Details

### Dependencies
- Compiled JavaScript from `../dist/index.js`
- No external dependencies required
- Pure vanilla JavaScript

### File Structure
```
html-demo/
├── index.html          # Main demo page
├── README.md           # This file
└── server.js           # Optional Node.js server
```

## Customization

### Adding New Event Buttons

Add a new button in the HTML:

```html
<button onclick="trackCustomEvent()">Custom Event</button>
```

Add the corresponding JavaScript function:

```javascript
function trackCustomEvent() {
    SudoQuery.track('custom_event', {
        customProperty: 'value',
        anotherProperty: 123
    });
    incrementEvents();
    logEvent('custom_event', 'Custom event tracked');
}
```

### Styling

The demo uses a modern gradient design with:
- Purple gradient theme
- Responsive layout
- Smooth animations
- Clean typography

## Troubleshooting

### Events Not Showing
- Check browser console for errors
- Verify `dist/index.js` exists
- Ensure JavaScript is enabled

### Beacon Not Working
- Beacon API requires HTTPS or localhost
- Check browser compatibility
- Some browsers may block beacon in certain contexts

### CORS Issues
- Use a local server (Python, Node.js, PHP)
- Avoid opening HTML file directly from file system

## Next Steps

1. **Integrate into Your App**: Copy the script tag and initialization code
2. **Customize Events**: Add event types specific to your application
3. **Set Up Backend**: Configure the API endpoint for event upload
4. **Add Authentication**: Integrate with your auth system for user tracking
5. **Monitor Events**: Set up analytics dashboard to visualize tracked events

## License

This demo is part of the SudoQuery project.