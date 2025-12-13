package com.arula.terminal;

import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.recyclerview.widget.DiffUtil;
import androidx.recyclerview.widget.ListAdapter;
import androidx.recyclerview.widget.RecyclerView;
import com.arula.terminal.databinding.ItemMessageBinding;
import java.text.SimpleDateFormat;
import java.util.*;

/**
 * Adapter for displaying chat messages
 */
public class MessageAdapter extends ListAdapter<Message, MessageAdapter.MessageViewHolder> {
    private static final DiffUtil.ItemCallback<Message> DIFF_CALLBACK = new DiffUtil.ItemCallback<Message>() {
        @Override
        public boolean areItemsTheSame(@NonNull Message oldItem, @NonNull Message newItem) {
            return oldItem.getId() == newItem.getId();
        }

        @Override
        public boolean areContentsTheSame(@NonNull Message oldItem, @NonNull Message newItem) {
            return oldItem.equals(newItem);
        }
    };

    private final Map<String, Integer> toolMessagePositions = new HashMap<>();
    private final SimpleDateFormat timeFormat = new SimpleDateFormat("HH:mm", Locale.getDefault());

    public MessageAdapter() {
        super(DIFF_CALLBACK);
    }

    @NonNull
    @Override
    public MessageViewHolder onCreateViewHolder(@NonNull ViewGroup parent, int viewType) {
        LayoutInflater inflater = LayoutInflater.from(parent.getContext());
        ItemMessageBinding binding = ItemMessageBinding.inflate(inflater, parent, false);
        return new MessageViewHolder(binding);
    }

    @Override
    public void onBindViewHolder(@NonNull MessageViewHolder holder, int position) {
        Message message = getItem(position);
        holder.bind(message);
    }

    public void appendToLastMessage(String chunk) {
        int lastPosition = getItemCount() - 1;
        if (lastPosition >= 0) {
            Message lastMessage = getItem(lastPosition);
            if (lastMessage.getType() == Message.Type.ASSISTANT) {
                lastMessage.appendText(chunk);
                notifyItemChanged(lastPosition);
            }
        }
    }

    public void updateToolMessage(String toolId, String result) {
        Integer position = toolMessagePositions.get(toolId);
        if (position != null && position < getItemCount()) {
            Message message = getItem(position);
            message.appendText("\n" + result);
            notifyItemChanged(position);
        }
    }

    static class MessageViewHolder extends RecyclerView.ViewHolder {
        private final ItemMessageBinding binding;

        public MessageViewHolder(ItemMessageBinding binding) {
            super(binding.getRoot());
            this.binding = binding;
        }

        public void bind(Message message) {
            // Set message content
            binding.messageText.setText(message.getText());

            // Set message appearance based on type
            switch (message.getType()) {
                case USER:
                    binding.messageCard.setCardBackgroundColor(itemView.getContext()
                        .getResources().getColor(android.R.color.holo_blue_light, null));
                    binding.senderText.setText("You");
                    binding.senderText.setTextColor(itemView.getContext()
                        .getResources().getColor(android.R.color.white, null));
                    break;

                case ASSISTANT:
                    binding.messageCard.setCardBackgroundColor(itemView.getContext()
                        .getResources().getColor(android.R.color.background_light, null));
                    binding.senderText.setText("Arula");
                    binding.senderText.setTextColor(itemView.getContext()
                        .getResources().getColor(android.R.color.primary_text_light, null));
                    break;

                case TOOL:
                    binding.messageCard.setCardBackgroundColor(itemView.getContext()
                        .getResources().getColor(android.R.color.holo_orange_light, null));
                    binding.senderText.setText("Tool");
                    binding.senderText.setTextColor(itemView.getContext()
                        .getResources().getColor(android.R.color.white, null));
                    break;

                case ERROR:
                    binding.messageCard.setCardBackgroundColor(itemView.getContext()
                        .getResources().getColor(android.R.color.holo_red_light, null));
                    binding.senderText.setText("Error");
                    binding.senderText.setTextColor(itemView.getContext()
                        .getResources().getColor(android.R.color.white, null));
                    break;
            }

            // Set timestamp
            if (message.getTimestamp() > 0) {
                String time = new SimpleDateFormat("HH:mm", Locale.getDefault())
                    .format(new Date(message.getTimestamp()));
                binding.timestampText.setText(time);
            } else {
                binding.timestampText.setText("");
            }
        }
    }
}