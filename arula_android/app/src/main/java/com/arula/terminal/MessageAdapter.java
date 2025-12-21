package com.arula.terminal;

import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.recyclerview.widget.RecyclerView;
import com.arula.terminal.databinding.ItemMessageNeonBinding;
import java.text.SimpleDateFormat;
import java.util.*;

/**
 * Adapter for displaying chat messages.
 * Uses standard RecyclerView.Adapter for full control over mutable message
 * updates.
 */
public class MessageAdapter extends RecyclerView.Adapter<MessageAdapter.MessageViewHolder> {

    private final Map<String, Integer> toolMessagePositions = new HashMap<>();
    private final SimpleDateFormat timeFormat = new SimpleDateFormat("HH:mm", Locale.getDefault());
    private final List<Message> messageList = new ArrayList<>();

    public MessageAdapter() {
    }

    @NonNull
    @Override
    public MessageViewHolder onCreateViewHolder(@NonNull ViewGroup parent, int viewType) {
        LayoutInflater inflater = LayoutInflater.from(parent.getContext());
        ItemMessageNeonBinding binding = ItemMessageNeonBinding.inflate(inflater, parent, false);
        return new MessageViewHolder(binding);
    }

    @Override
    public void onBindViewHolder(@NonNull MessageViewHolder holder, int position) {
        Message message = messageList.get(position);
        holder.bind(message);
    }

    @Override
    public int getItemCount() {
        return messageList.size();
    }

    public Message getItem(int position) {
        if (position >= 0 && position < messageList.size()) {
            return messageList.get(position);
        }
        return null;
    }

    public void setMessages(List<Message> messages) {
        messageList.clear();
        messageList.addAll(messages);
        notifyDataSetChanged();
    }

    public void addMessage(Message message) {
        messageList.add(message);
        notifyItemInserted(messageList.size() - 1);
    }

    public void clearMessages() {
        int size = messageList.size();
        messageList.clear();
        notifyItemRangeRemoved(0, size);
    }

    /**
     * Updates the last message in the list and forces the adapter to rebind.
     */
    public void updateLastMessage() {
        if (!messageList.isEmpty()) {
            int lastPos = messageList.size() - 1;
            notifyItemChanged(lastPos);
        }
    }

    public void appendToLastMessage(String chunk) {
        if (!messageList.isEmpty()) {
            int lastPosition = messageList.size() - 1;
            Message lastMessage = messageList.get(lastPosition);
            if (lastMessage.getType() == Message.Type.ASSISTANT) {
                lastMessage.appendText(chunk);
                notifyItemChanged(lastPosition);
            }
        }
    }

    public void removeLastMessage() {
        if (!messageList.isEmpty()) {
            int lastPos = messageList.size() - 1;
            messageList.remove(lastPos);
            notifyItemRemoved(lastPos);
        }
    }

    public void updateToolMessage(String toolId, String result) {
        Integer position = toolMessagePositions.get(toolId);
        if (position != null && position < messageList.size()) {
            Message message = messageList.get(position);
            message.appendText("\n" + result);
            notifyItemChanged(position);
        }
    }

    static class MessageViewHolder extends RecyclerView.ViewHolder {
        private final ItemMessageNeonBinding binding;

        public MessageViewHolder(ItemMessageNeonBinding binding) {
            super(binding.getRoot());
            this.binding = binding;
        }

        public void bind(Message message) {
            // Set message content
            binding.messageText.setText(message.getText());

            android.content.Context ctx = itemView.getContext();

            // Set message appearance based on type
            switch (message.getType()) {
                case USER:
                    binding.messageCard.setCardBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_accent, null));
                    binding.senderText.setText("You");
                    binding.senderText.setTextColor(
                            ctx.getResources().getColor(R.color.neon_text, null));
                    binding.senderIndicator.setBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_success, null));
                    // Apply gradient background
                    binding.messageCard.setBackgroundResource(R.drawable.bg_user_message);
                    break;

                case ASSISTANT:
                    binding.messageCard.setCardBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_surface_raised, null));
                    binding.senderText.setText("Arula");
                    binding.senderText.setTextColor(
                            ctx.getResources().getColor(R.color.neon_text, null));
                    binding.senderIndicator.setBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_accent, null));
                    // Apply surface background
                    binding.messageCard.setBackgroundResource(R.drawable.bg_assistant_message);
                    break;

                case TOOL:
                    binding.messageCard.setCardBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_tool_bubble, null));
                    binding.senderText.setText("Tool");
                    binding.senderText.setTextColor(
                            ctx.getResources().getColor(R.color.neon_text, null));
                    binding.senderIndicator.setBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_success, null));
                    // Apply tool background
                    binding.messageCard.setBackgroundResource(R.drawable.bg_tool_message);
                    break;

                case ERROR:
                    binding.messageCard.setCardBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_danger, null));
                    binding.senderText.setText("Error");
                    binding.senderText.setTextColor(
                            ctx.getResources().getColor(R.color.neon_text, null));
                    binding.senderIndicator.setBackgroundColor(
                            ctx.getResources().getColor(R.color.neon_danger, null));
                    break;
            }

            // Set neon styling for text
            binding.messageText.setTextColor(
                    ctx.getResources().getColor(R.color.neon_text, null));
            binding.timestampText.setTextColor(
                    ctx.getResources().getColor(R.color.neon_muted, null));

            // Set timestamp
            if (message.getTimestamp() > 0) {
                String time = new SimpleDateFormat("HH:mm", Locale.getDefault())
                        .format(new Date(message.getTimestamp()));
                binding.timestampText.setText(time);
            } else {
                binding.timestampText.setText("");
            }

            // Add neon glow effect for messages
            if (message.getType() == Message.Type.USER || message.getType() == Message.Type.ASSISTANT) {
                binding.messageCard.setElevation(4f);
            } else {
                binding.messageCard.setElevation(0f);
            }
        }
    }
}